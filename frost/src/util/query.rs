use std::collections::HashSet;

use crate::errors::Error;
use crate::time::Time;
use crate::{ConnectionID, DecompressedBag, IndexData, MessageDataHeader};

use super::{msgs::MessageView, parsing::parse_le_u32_at};

pub struct Query {
    topics: Option<Vec<String>>,
    types: Option<Vec<String>>,
    start_time: Option<Time>,
    end_time: Option<Time>,
}

impl Query {
    /// Query all messages in a bag.
    pub fn all() -> Self {
        Query {
            topics: None,
            types: None,
            start_time: None,
            end_time: None,
        }
    }

    /// Create a Query.
    pub fn new() -> Self {
        Self::all()
    }

    /// Query a bag with specific Topics.
    pub fn with_topics<S, I>(mut self, topics: I) -> Self
    where
        S: AsRef<str>,
        I: IntoIterator<Item = S>,
    {
        self.topics = Some(topics.into_iter().map(|s| s.as_ref().into()).collect());
        self
    }

    /// Query a bag with specific message Types.
    pub fn with_types<S, I>(mut self, types: I) -> Self
    where
        S: AsRef<str>,
        I: IntoIterator<Item = S>,
    {
        self.types = Some(types.into_iter().map(|s| s.as_ref().into()).collect());
        self
    }

    /// Query a bag with messages filtered after a start time.
    pub fn with_start_time(mut self, start_time: Time) -> Self {
        self.start_time = Some(start_time);
        self
    }

    /// Query a bag with messages filtered before an end time.
    pub fn with_end_time(mut self, end_time: Time) -> Self {
        self.end_time = Some(end_time);
        self
    }
}

impl Default for Query {
    fn default() -> Self {
        Self::new()
    }
}

pub struct BagIter<'a> {
    bag: &'a DecompressedBag,
    index_data: Vec<IndexData>,
    current_index: usize,
}
impl<'a> BagIter<'a> {
    pub(crate) fn new(bag: &'a DecompressedBag, query: &Query) -> Result<Self, Error> {
        let topic_to_connection_ids = bag.metadata.topic_to_connection_ids();
        let ids_from_topics: HashSet<ConnectionID> = match &query.topics {
            Some(topics) => topics
                .iter()
                .flat_map(|topic| topic_to_connection_ids.get(topic).cloned())
                .flatten()
                .collect(),
            None => topic_to_connection_ids
                .values()
                .flatten()
                .cloned()
                .collect(),
        };
        let types_to_connection_ids = bag.metadata.type_to_connection_ids();
        let ids_from_types: HashSet<ConnectionID> = match &query.types {
            Some(types) => types
                .iter()
                .flat_map(|ty| types_to_connection_ids.get(ty).cloned())
                .flatten()
                .collect(),
            None => types_to_connection_ids
                .values()
                .flatten()
                .cloned()
                .collect(),
        };
        let ids: HashSet<ConnectionID> = ids_from_topics
            .intersection(&ids_from_types)
            .cloned()
            .collect();
        let mut index_data: Vec<IndexData> = ids
            .iter()
            .flat_map(|id| bag.metadata.index_data.get(id).unwrap().clone())
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

            let topic = &self
                .bag
                .metadata
                .connection_data
                .get(&data.conn_id)
                .unwrap()
                .topic;

            let chunk_bytes = self.bag.chunk_bytes.get(&data.chunk_header_pos)?;

            let mut pos = data.offset;

            let header_len = parse_le_u32_at(chunk_bytes, pos).unwrap() as usize;
            pos += 4;
            let header_start = pos;
            let header_end = header_start + header_len;

            MessageDataHeader::from(&chunk_bytes[header_start..header_end])
                .expect("Failed to read MessageDataHeader");
            pos = header_end;

            let data_len = parse_le_u32_at(chunk_bytes, pos).unwrap() as usize;
            // serde_rosmsg wants the data_len included, so don't pos += 4;
            let data_start = pos;
            let data_end = data_start + data_len + 4; // add extra 4 for data_len

            self.current_index += 1;

            Some(MessageView {
                topic,
                chunk_loc: data.chunk_header_pos,
                bag: self.bag,
                start_index: data_start,
                end_index: data_end,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Query;
    use itertools::assert_equal;
    use itertools::sorted;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn test_contruction_with_topics() {
        let query = Query::new().with_topics(["/chatter", "/array"]);
        assert_equal(sorted(query.topics.unwrap()), ["/array", "/chatter"]);
        assert_eq!(query.start_time, None);
        assert_eq!(query.end_time, None);

        let query = Query::new().with_topics(["/chatter", "/array"]);
        assert_equal(sorted(query.topics.unwrap()), ["/array", "/chatter"]);
        assert_eq!(query.start_time, None);
        assert_eq!(query.end_time, None);

        let topics: HashSet<&str> = HashSet::from_iter(["/chatter", "/array"]);
        let query = Query::new().with_topics(topics);
        assert_equal(sorted(query.topics.unwrap()), ["/array", "/chatter"]);
        assert_eq!(query.start_time, None);
        assert_eq!(query.end_time, None);

        let topics: HashMap<&str, u32> = HashMap::from_iter([("/chatter", 0), ("/array", 0)]);
        let query = Query::new().with_topics(topics.keys());
        assert_equal(sorted(query.topics.unwrap()), ["/array", "/chatter"]);
        assert_eq!(query.start_time, None);
        assert_eq!(query.end_time, None);

        let topics = vec!["/chatter", "/array"];
        let query = Query::new().with_topics(&topics);
        assert_equal(sorted(query.topics.unwrap()), ["/array", "/chatter"]);

        let query = Query::new().with_topics(topics);
        assert_equal(sorted(query.topics.unwrap()), ["/array", "/chatter"]);
    }
}
