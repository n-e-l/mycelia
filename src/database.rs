use std::sync::Arc;
use neo4rs::{Graph, Node, query};

pub struct Database {
    graph: Arc<Graph>,
}

impl Database {
    pub async fn new() -> Database {
        let uri = std::env::var("NEO_URI").expect("Can't read env var");
        let username = std::env::var("NEO_USERNAME").expect("Can't read env var");
        let password = std::env::var("NEO_PASSWORD").expect("Can't read env var");
        let graph = Arc::new(Graph::new(uri, username, password).await.unwrap());

        Database {
            graph
        }
    }

    pub async fn get_concepts(&self) -> Vec<String> {
        let mut result = self.graph.execute(
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

    pub async fn get_messages(&self) -> Vec<String> {
        let mut result = self.graph.execute(
            query( "MATCH (n:Message) RETURN n")
        ).await.unwrap();

        let mut concepts: Vec<String> = Vec::new();
        while let Ok(Some(row)) = result.next().await {
            let node: Node = row.get("n").unwrap();
            let name: String = node.get("text").unwrap();
            concepts.push(name);
        }

        concepts
    }
}