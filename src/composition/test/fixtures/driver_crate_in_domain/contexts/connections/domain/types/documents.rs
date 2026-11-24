use bson::Document;
use mongodb::Client;

pub struct Documents(Client, Document);
