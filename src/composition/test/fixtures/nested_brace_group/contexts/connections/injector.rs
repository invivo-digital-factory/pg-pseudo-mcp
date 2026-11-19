use crate::{
    common::AppConfig,
    contexts::{connections::domain::Session, querying::Report},
};

pub struct Injector(AppConfig, Session, Report);
