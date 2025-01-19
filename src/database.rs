use std::sync::Arc;
use neo4rs::{Graph, Node, query};

pub struct Database {
    uri: String,
    username: String,
    password: String,
}

impl Database {
    pub fn new() -> Database {
        let uri = std::env::var("NEO_URI").expect("Can't read env var");
        let username = std::env::var("NEO_USERNAME").expect("Can't read env var");
        let password = std::env::var("NEO_PASSWORD").expect("Can't read env var");

        Database {
            uri: uri.parse().unwrap(),
            username: username.parse().unwrap(),
            password: password.parse().unwrap(),
        }
    }

    pub async fn graph(&self) -> Arc<Graph> {
        Arc::new(Graph::new(&self.uri, &self.username, &self.password).await.unwrap())
    }

    pub async fn get_concepts(&self) -> Vec<String> {
        let mut result = self.graph().await.execute(
            query( "MATCH (n:Concept) RETURN n")
        ).await.unwrap();

        let mut concepts: Vec<String> = Vec::new();
        while let Ok(Some(row)) = result.next().await {
            let node: Node = row.get("n").unwrap();
            let name: String = node.get("name").unwrap();
            concepts.push(name);
        }

        concepts
    }
}