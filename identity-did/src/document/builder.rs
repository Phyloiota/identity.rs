// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use identity_core::common::Url;

use crate::did::DID;
use crate::document::Document;
use crate::error::Result;
use crate::service::Service;
use crate::utils::DIDKey;
use crate::verification::Method;
use crate::verification::MethodRef;

/// A `Builder` is used to generate a customized `Document`.
#[derive(Clone, Debug)]
pub struct Builder<T = (), U = (), V = ()> {
  pub(crate) id: Option<DID>,
  pub(crate) controller: Option<DID>,
  pub(crate) also_known_as: Vec<Url>,
  pub(crate) verification_method: Vec<DIDKey<Method<U>>>,
  pub(crate) authentication: Vec<DIDKey<MethodRef<U>>>,
  pub(crate) assertion_method: Vec<DIDKey<MethodRef<U>>>,
  pub(crate) key_agreement: Vec<DIDKey<MethodRef<U>>>,
  pub(crate) capability_delegation: Vec<DIDKey<MethodRef<U>>>,
  pub(crate) capability_invocation: Vec<DIDKey<MethodRef<U>>>,
  pub(crate) service: Vec<DIDKey<Service<V>>>,
  pub(crate) properties: T,
}

impl<T, U, V> Builder<T, U, V> {
  /// Creates a new `Builder`.
  pub fn new(properties: T) -> Self {
    Self {
      id: None,
      controller: None,
      also_known_as: Vec::new(),
      verification_method: Vec::new(),
      authentication: Vec::new(),
      assertion_method: Vec::new(),
      key_agreement: Vec::new(),
      capability_delegation: Vec::new(),
      capability_invocation: Vec::new(),
      service: Vec::new(),
      properties,
    }
  }

  /// Sets the `id` value of the generated `Document`.
  #[must_use]
  pub fn id(mut self, value: DID) -> Self {
    self.id = Some(value);
    self
  }

  /// Sets the `controller` value of the generated `Document`.
  #[must_use]
  pub fn controller(mut self, value: DID) -> Self {
    self.controller = Some(value);
    self
  }

  /// Adds a value to the `alsoKnownAs` set of the generated `Document`.
  #[must_use]
  pub fn also_known_as(mut self, value: Url) -> Self {
    self.also_known_as.push(value);
    self
  }

  /// Adds a value to the `verificationMethod` set of the generated `Document`.
  #[must_use]
  pub fn verification_method(mut self, value: Method<U>) -> Self {
    self.verification_method.push(DIDKey::new(value));
    self
  }

  /// Adds a value to the `authentication` set of the generated `Document`.
  #[must_use]
  pub fn authentication(mut self, value: impl Into<MethodRef<U>>) -> Self {
    self.authentication.push(DIDKey::new(value.into()));
    self
  }

  /// Adds a value to the `assertionMethod` set of the generated `Document`.
  #[must_use]
  pub fn assertion_method(mut self, value: impl Into<MethodRef<U>>) -> Self {
    self.assertion_method.push(DIDKey::new(value.into()));
    self
  }

  /// Adds a value to the `keyAgreement` set of the generated `Document`.
  #[must_use]
  pub fn key_agreement(mut self, value: impl Into<MethodRef<U>>) -> Self {
    self.key_agreement.push(DIDKey::new(value.into()));
    self
  }

  /// Adds a value to the `capabilityDelegation` set of the generated `Document`.
  #[must_use]
  pub fn capability_delegation(mut self, value: impl Into<MethodRef<U>>) -> Self {
    self.capability_delegation.push(DIDKey::new(value.into()));
    self
  }

  /// Adds a value to the `capabilityInvocation` set of the generated `Document`.
  #[must_use]
  pub fn capability_invocation(mut self, value: impl Into<MethodRef<U>>) -> Self {
    self.capability_invocation.push(DIDKey::new(value.into()));
    self
  }

  /// Adds a value to the `service` set of the generated `Document`.
  #[must_use]
  pub fn service(mut self, value: Service<V>) -> Self {
    self.service.push(DIDKey::new(value));
    self
  }

  /// Returns a new `Document` based on the `Builder` configuration.
  pub fn build(self) -> Result<Document<T, U, V>> {
    Document::from_builder(self)
  }
}

impl<T, U, V> Default for Builder<T, U, V>
where
  T: Default,
{
  fn default() -> Self {
    Self::new(T::default())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  #[should_panic = "InvalidDocumentId"]
  fn test_missing_id() {
    let _: Document = Builder::default().build().unwrap();
  }
}
