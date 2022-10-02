use std::collections::HashSet;

use crate::errors::FrostError;
use crate::time::Time;
use crate::{Bag, ConnectionID, IndexData, MessageDataHeader};

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

    pub fn with_topics<S>(mut self, topics: &[S]) -> Self
    where
        S: AsRef<str> + Into<String>,
    {
        self.topics = Some(topics.iter().map(|s| s.as_ref().into()).collect());
        self
    }

    pub fn with_start_time(mut self, start_time: Time) -> Self {
        self.start_time = Some(start_time);
        self
    }

    pub fn with_end_time(mut self, end_time: Time) -> Self {
        self.end_time = Some(end_time);
        self
    }
}

pub struct BagIter<'a> {
    bag: &'a Bag,
    index_data: Vec<IndexData>,
    current_index: usize,
}
impl<'a> BagIter<'a> {
    pub(crate) fn new(bag: &'a mut Bag, query: &Query) -> Result<Self, FrostError> {
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

        bag.populate_chunk_bytes()?;

        Ok(BagIter {
            bag,
            index_data,
            current_index: 0,
        })
    }
}

impl<'a> Iterator for BagIter<'a> {
    type Item = MessageView<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index >= self.index_data.len() {
            None
        } else {
            let data = self.index_data.get(self.current_index)?;

            let topic = &self.bag.connection_data.get(&data.conn_id).unwrap().topic;

            let chunk_bytes = self.bag.chunk_bytes.get(&data.chunk_header_pos)?;

            let mut pos = data.offset;

            let header_len = parse_le_u32_at(&chunk_bytes, pos).unwrap() as usize;
            pos += 4;
            let header_start = pos;
            let header_end = header_start + header_len;

            MessageDataHeader::from(&chunk_bytes[header_start..header_end])
                .expect("Failed to read MessageDataHeader");
            pos = header_end;

            let data_len = parse_le_u32_at(&chunk_bytes, pos).unwrap() as usize;
            // serde_rosmsg wants the data_len included, so don't pos += 4;
            let data_start = pos;
            let data_end = data_start + data_len + 4; // add extra 4 for data_len

            self.current_index += 1;

            Some(MessageView {
                topic,
                chunk_loc: data.chunk_header_pos,
                bag: &self.bag,
                start_index: data_start,
                end_index: data_end,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Query;

    #[test]
    fn test_contruction_with_topics() {
        let query = Query::new().with_topics(&["/chatter", "array"]);
        assert_eq!(
            query.topics,
            Some(vec!("/chatter".to_string(), "array".to_string()))
        );
        assert_eq!(query.start_time, None);
        assert_eq!(query.end_time, None);

        let query = Query::new().with_topics(&vec!["/chatter", "array"]);
        assert_eq!(
            query.topics,
            Some(vec!("/chatter".to_string(), "array".to_string()))
        );
        assert_eq!(query.start_time, None);
        assert_eq!(query.end_time, None);
    }
}
