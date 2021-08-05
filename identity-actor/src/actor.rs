// Copyright 2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{any::Any, ops::Deref, sync::Arc};

use crate::{
  asyncfn::AsyncFn,
  errors::{Error, RemoteSendError, Result, SendError},
  traits::{ActorRequest, RequestHandler},
  types::NamedMessage,
};
use dashmap::DashMap;
use futures::{channel::mpsc, Future, StreamExt};
use libp2p::{Multiaddr, PeerId};
use p2p::{ListenErr, ReceiveRequest, StrongholdP2p};
use serde_json::error::Category;
use tokio::task::{self, JoinHandle};
use uuid::Uuid;

pub struct HandlerBuilder {
  object_id: Uuid,
  handlers: Arc<DashMap<String, (Uuid, Box<dyn RequestHandler>)>>,
}

impl HandlerBuilder {
  pub fn add_method<H, R, F, C>(self, cmd: &'static str, handler: AsyncFn<H, R, F, C>) -> Self
  where
    H: Clone + Send + Sync,
    R: ActorRequest + Send + Sync + 'static,
    F: Future<Output = R::Response> + Send + 'static,
    C: 'static + Send + Sync + Fn(H, R) -> F,
  {
    self.handlers.insert(cmd.into(), (self.object_id, Box::new(handler)));
    self
  }
}

pub struct Actor {
  comm: StrongholdP2p<NamedMessage, NamedMessage, NamedMessage>,
  handlers: Arc<DashMap<String, (Uuid, Box<dyn RequestHandler>)>>,
  objects: Arc<DashMap<Uuid, Box<dyn Any + Send + Sync + 'static>>>,
  listener_handle: Option<JoinHandle<Result<()>>>,
}

impl Actor {
  pub(crate) async fn from_builder(
    receiver: mpsc::Receiver<ReceiveRequest<NamedMessage, NamedMessage>>,
    mut comm: StrongholdP2p<NamedMessage, NamedMessage, NamedMessage>,
    handlers: DashMap<String, (Uuid, Box<dyn RequestHandler>)>,
    objects: DashMap<Uuid, Box<dyn Any + Send + Sync + 'static>>,
    listening_addresses: Vec<Multiaddr>,
  ) -> Result<Self> {
    let handlers = Arc::new(handlers);
    let objects = Arc::new(objects);

    let listener_handle = if !listening_addresses.is_empty() {
      Some(Self::spawn_listener(
        receiver,
        Arc::clone(&objects),
        Arc::clone(&handlers),
      ))
    } else {
      None
    };

    for addr in listening_addresses {
      comm.start_listening(addr).await?;
    }

    Ok(Self {
      comm,
      handlers,
      objects,
      listener_handle,
    })
  }

  pub fn add_handler<H>(&mut self, handler: H) -> HandlerBuilder
  where
    H: Clone + Any + Send + Sync,
  {
    let object_id = Uuid::new_v4();
    self.objects.insert(object_id, Box::new(handler));
    HandlerBuilder {
      object_id,
      handlers: Arc::clone(&self.handlers),
    }
  }

  pub async fn start_listening(&mut self, address: Multiaddr) -> std::result::Result<Multiaddr, ListenErr> {
    self.comm.start_listening(address).await
  }

  pub fn peer_id(&self) -> PeerId {
    self.comm.get_peer_id()
  }

  pub async fn stop_listening(&mut self) {
    self.comm.stop_listening().await;
  }

  pub async fn addrs(&mut self) -> Vec<Multiaddr> {
    let listeners = self.comm.get_listeners().await;
    listeners.into_iter().map(|l| l.addrs).flatten().collect()
  }

  /// Start handling incoming requests. This method does not return unless [`stop_listening`] is called.
  /// This method should only be called once on any given instance.
  /// A second caller would immediately receive an [`Error::LockInUse`].
  fn spawn_listener(
    mut receiver: mpsc::Receiver<ReceiveRequest<NamedMessage, NamedMessage>>,
    objects: Arc<DashMap<Uuid, Box<dyn Any + Send + Sync>>>,
    handlers: Arc<DashMap<String, (Uuid, Box<dyn RequestHandler>)>>,
  ) -> JoinHandle<Result<()>> {
    task::spawn(async move {
      let mut handles = vec![];
      loop {
        if let Some(receive_request) = receiver.next().await {
          let handler_handle = Self::spawn_handler(receive_request, Arc::clone(&objects), Arc::clone(&handlers));
          handles.push(handler_handle);
        } else {
          return Ok(());
        }
      }
    })
  }

  fn spawn_handler(
    receive_request: ReceiveRequest<NamedMessage, NamedMessage>,
    objects: Arc<DashMap<Uuid, Box<dyn Any + Send + Sync>>>,
    handlers: Arc<DashMap<String, (Uuid, Box<dyn RequestHandler>)>>,
  ) -> JoinHandle<Result<()>> {
    task::spawn(async move {
      let request = receive_request.request;
      let response_tx = receive_request.response_tx;
      let request_name = request.name;

      let response_data = match handlers.get(&request_name) {
        Some(handler_tuple) => {
          let object_id = handler_tuple.0;
          let handler = &handler_tuple.1;

          let obj = if let Some(object) = objects.get(&object_id) {
            let object_clone = handler.clone_object(object.deref());
            object_clone
          } else {
            todo!("Unhandled object-not-found case.")
          };

          handler.invoke(obj, request.data).await
        }
        // TODO: Send error *back to peer*
        None => {
          // SAFETY: Serialization never fails
          serde_json::to_vec(&RemoteSendError::UnknownRequest(request_name)).unwrap()
        }
      };

      let response = NamedMessage::new("temporary_solution", response_data);

      // TODO: can return an error when
      // - connection times out, when
      // - when handler takes too long to respond (configurable via SwarmBuilder.with_timeout)
      // - error on the transport layer
      // - potentially others...
      response_tx
        .send(response)
        .map_err(|response| Error::CouldNotRespond(response.name))
    })
  }

  pub async fn stop_handling_requests(self) -> Result<()> {
    // TODO: aborting means that even requests that have been received and are being processed are cancelled
    // We should instead use some signalling mechanism that breaks the loop
    if let Some(listener_handle) = self.listener_handle {
      listener_handle.abort();
      let _ = listener_handle.await;
    }
    Ok(())
  }

  pub async fn add_peer(&mut self, peer: PeerId, addr: Multiaddr) {
    self.comm.add_address(peer, addr).await;
  }

  pub async fn send_request<Request: ActorRequest>(
    &mut self,
    peer: PeerId,
    command: Request,
  ) -> std::result::Result<Request::Response, SendError> {
    self.send_named_request(peer, &*command.request_name(), command).await
  }

  pub async fn send_named_request<Request: ActorRequest>(
    &mut self,
    peer: PeerId,
    name: &str,
    command: Request,
  ) -> std::result::Result<Request::Response, SendError> {
    let request = NamedMessage::new(name, serde_json::to_vec(&command).unwrap());

    log::info!(
      "Sending `{}` request with payload: {}",
      request.name,
      serde_json::to_string_pretty(&command).unwrap()
    );

    let response = self.comm.send_request(peer, request).await.unwrap();

    log::info!("Response: {}", std::str::from_utf8(&response.data).unwrap());

    let request_response: serde_json::Result<Request::Response> = serde_json::from_slice(&response.data);

    match request_response {
      Ok(res) => Ok(res),
      Err(err) => match err.classify() {
        // If we failed to deserialize into the expected response, due to syntactically or
        // semantically incorrect bytes, we attempt deserialization into a `RemoteSendError`.
        Category::Data | Category::Syntax => {
          let remote_send_err: serde_json::Result<RemoteSendError> = serde_json::from_slice(&response.data);

          match remote_send_err {
            Ok(remote_err) => Err(SendError::from(remote_err)),
            Err(err) => Err(SendError::ResponseDeserializationFailure(err.to_string())),
          }
        }
        _ => Err(SendError::ResponseDeserializationFailure(err.to_string())),
      },
    }
  }

  pub async fn join(self) {
    if let Some(listener_handle) = self.listener_handle {
      listener_handle.await.unwrap().unwrap();
    }
  }
}
