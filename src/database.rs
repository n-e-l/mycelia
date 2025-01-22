use std::sync::Arc;
use neo4rs::{Graph, Node, query};

pub struct Concept {
    pub id: usize,
    pub name: String,
}

pub struct Relation {
    pub id: usize,
    pub a: usize,
    pub b: usize,
}

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

    pub async fn get_concepts(&self) -> Vec<Concept> {
        let mut result = self.graph.execute(
            query( "MATCH (n:Concept) RETURN n,id(n)")
        ).await.unwrap();

        let mut concepts: Vec<Concept> = Vec::new();
        while let Ok(Some(row)) = result.next().await {
            let node: Node = row.get("n").unwrap();
            let name: String = node.get("name").unwrap();
            let id: usize = row.get("id(n)").unwrap();
            concepts.push(Concept { id, name });
        }

        concepts
    }

    pub async fn get_relations(&self) -> Vec<Relation> {
        let mut result = self.graph.execute(
            query( "MATCH (a)-[r]->(b) RETURN id(a),id(r),id(b)")
        ).await.unwrap();

        let mut relations: Vec<Relation> = Vec::new();
        while let Ok(Some(row)) = result.next().await {
            let id: usize = row.get("id(r)").unwrap();
            let a: usize = row.get("id(a)").unwrap();
            let b: usize = row.get("id(b)").unwrap();
            relations.push(Relation {
                id,
                a,
                b
            });
        }

        relations
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