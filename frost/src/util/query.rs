use std::collections::HashSet;

use crate::{Bag, ConnectionID, IndexData};

use super::{msgs::MessageView, time::Time};

pub struct Query {
    topics: Option<Vec<String>>,
    start_time: Option<Time>,
    end_time: Option<Time>,
}

impl Query {
    pub fn all() -> Self {
        Query {
            topics: None,
            start_time: None,
            end_time: None,
        }
    }

    pub fn new() -> Self {
        Self::all()
    }

    pub fn with_topics<S>(&mut self, topics: &Vec<S>) -> &mut Self
    where
        S: AsRef<str> + Into<String>,
    {
        self.topics = Some(topics.iter().map(|s| s.as_ref().into()).collect());
        self
    }

    pub fn with_start_time(&mut self, start_time: Time) -> &mut Self {
        self.start_time = Some(start_time);
        self
    }

    pub fn with_end_time(&mut self, end_time: Time) -> &mut Self {
        self.end_time = Some(end_time);
        self
    }
}

pub struct BagIter<'a> {
    bag: &'a Bag,
    index_data: Vec<&'a IndexData>,
    current_pos: usize,
}
impl<'a> BagIter<'a> {
    pub(crate) fn new(bag: &'a Bag, query: &Query) -> Self {
        let ids: HashSet<ConnectionID> = match &query.topics {
            Some(topics) => topics
                .iter()
                .flat_map(|topic| bag.topic_to_connection_ids.get(topic).map(|v| v.clone()))
                .flatten()
                .collect(),
            None => bag
                .topic_to_connection_ids
                .values()
                .map(|v| v.clone())
                .flatten()
                .collect(),
        };

        let mut index_data: Vec<&IndexData> = ids
            .iter()
            .map(|id| bag.index_data.get(id).unwrap())
            .flatten()
            .filter(|data| {
                if let Some(start_time) = query.start_time {
                    if data.time < start_time {
                        return false;
                    }
                }
                if let Some(end_time) = query.end_time {
                    if data.time > end_time {
                        return false;
                    }
                }
                true
            })
            .collect();
        index_data.sort_by(|a, b| a.time.cmp(&b.time));
        BagIter {
            bag,
            index_data,
            current_pos: 0,
        }
    }
    fn get_index_data(&mut self) -> Vec<&'a IndexData> {
        todo!()
    }
}
impl<'a> Iterator for BagIter<'a> {
    type Item = MessageView;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}
