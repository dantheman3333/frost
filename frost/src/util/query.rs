use super::time::Time;

pub struct Query {
    topics: Option<Vec<String>>,
    start_time: Option<Time>,
    end_time: Option<Time>,
}

impl Query {
    fn all() -> Self {
        Query {
            topics: None,
            start_time: None,
            end_time: None,
        }
    }

    fn new() -> Self {
        Self::all()
    }

    fn with_topics<S>(&mut self, topics: &Vec<S>) -> &mut Self
    where
        S: AsRef<str> + Into<String>,
    {
        self.topics = Some(topics.iter().map(|s| s.as_ref().into()).collect());
        self
    }

    fn with_start_time(&mut self, start_time: Time) -> &mut Self {
        self.start_time = Some(start_time);
        self
    }

    fn with_end_time(&mut self, end_time: Time) -> &mut Self {
        self.end_time = Some(end_time);
        self
    }
}
