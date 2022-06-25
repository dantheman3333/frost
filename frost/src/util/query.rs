use std::collections::HashSet;

use crate::{std_msgs::std_msgs::Time, Bag, ConnectionID, IndexData};

use super::{msgs::MessageView, parsing::parse_le_u32_at};

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

    pub fn build(&self) -> Self {
        Query {
            topics: self.topics.clone(),
            start_time: self.start_time,
            end_time: self.end_time,
        }
    }
}

pub struct BagIter<'a> {
    bag: &'a mut Bag,
    index_data: Vec<IndexData>,
    current_pos: usize,
}
impl<'a> BagIter<'a> {
    pub(crate) fn new(bag: &'a mut Bag, query: &Query) -> Self {
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

        let mut index_data: Vec<IndexData> = ids
            .iter()
            .map(|id| bag.index_data.get(id).unwrap().clone())
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
}

impl<'a> Iterator for BagIter<'a> {
    type Item = MessageView;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_pos >= self.index_data.len() {
            None
        } else {
            let data = self.index_data.get(self.current_pos).unwrap().clone();
            dbg!(data.clone());
            let chunk_bytes = self.bag.get_chunk_bytes(data.chunk_header_pos);
            let msg_size = parse_le_u32_at(&chunk_bytes, data.offset).unwrap();
            self.current_pos += 1;
            Some(MessageView {
                bytes: chunk_bytes[data.offset..data.offset + msg_size as usize].to_vec(),
            })
        }
    }
}
