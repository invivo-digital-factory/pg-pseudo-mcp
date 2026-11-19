use super::Input;
use crate::contexts::connections::domain::{errors::Failure, types::Session};

pub struct OpenSession(Input, Failure, Session);

#[cfg(test)]
mod tests {
    use crate::contexts::querying::Report;
}
