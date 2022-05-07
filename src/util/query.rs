use crate::Bag;

use super::time::Time;


struct Query {
    topics: Option<Vec<String>>,
    types: Option<Vec<String>>,
    start_time: Option<Time>,
    end_time: Option<Time>
}

impl Query {
    pub fn new() -> Query {
        Query { topics: None, types: None, start_time: None, end_time: None }
    }
    pub fn all() -> Query {
        Query::new()
    }
    pub fn topics<'a>(&'a mut self, topics: Vec<String>) -> &'a mut Query {
        self.topics = Some(topics);
        self
    }
    pub fn types<'a>(&'a mut self, types: Vec<String>) -> &'a mut Query {
        self.topics = Some(types);
        self
    }
    pub fn start_time<'a>(&'a mut self, start_time: Time) -> &'a mut Query {
        self.start_time = Some(start_time);
        self
    }
    pub fn end_time<'a>(&'a mut self, end_time: Time) -> &'a mut Query {
        self.end_time = Some(end_time);
        self
    }
}

struct BagIter<'a> {
    bag: &'a Bag,
    query: Query
}

impl Iterator for BagIter<'_> {
    type Item;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}