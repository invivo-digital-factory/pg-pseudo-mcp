use std::cell::RefCell;
use std::path::Path;

use super::pgpass;
use crate::contexts::connections::domain::errors::AccessTokenError;
use crate::contexts::connections::domain::repositories::AzureSignInTokenProvider;
use crate::contexts::connections::domain::types::{ConnectionStatus, PasswordSource, Secret};

const PGPASS_REASON: &str = "no matching entry in the pgpass file";

pub(crate) struct SignIn<'a> {
    tokens: &'a dyn AzureSignInTokenProvider,
    attempt: RefCell<Option<Result<Secret, AccessTokenError>>>,
}

impl<'a> SignIn<'a> {
    pub(crate) fn new(tokens: &'a dyn AzureSignInTokenProvider) -> Self {
        Self {
            tokens,
            attempt: RefCell::new(None),
        }
    }

    fn token(&self) -> Result<Secret, AccessTokenError> {
        self.attempt
            .borrow_mut()
            .get_or_insert_with(|| self.tokens.access_token())
            .clone()
    }
}

pub(crate) fn pgpass_or_azure_token(
    target: &pgpass::Target<'_>,
    azure: bool,
    pgpass_file: Option<&Path>,
    sign_in: &SignIn<'_>,
) -> Result<PasswordSource, ConnectionStatus> {
    let unavailable = |reason: String| ConnectionStatus::Unavailable { reason };
    let from_pgpass = || -> Result<Option<Secret>, ConnectionStatus> {
        match pgpass_file {
            Some(path) => pgpass::lookup(path, target).map_err(unavailable),
            None => Ok(None),
        }
    };
    if !azure {
        return from_pgpass()?
            .map(PasswordSource::Resolved)
            .ok_or_else(|| unavailable(PGPASS_REASON.to_string()));
    }
    match sign_in.token() {
        Ok(token) => Ok(PasswordSource::AzureToken(token)),
        Err(sign_in_error) => from_pgpass()?
            .map(PasswordSource::Resolved)
            .ok_or_else(|| unavailable(sign_in_error.to_string())),
    }
}
