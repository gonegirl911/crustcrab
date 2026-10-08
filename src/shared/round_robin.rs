use std::iter;

pub struct RoundRobin<T> {
    last: Option<T>,
}

impl<T> RoundRobin<T> {
    pub fn advance(&mut self, last: T) {
        self.last = Some(last);
    }
}

impl<T: PartialEq> RoundRobin<T> {
    pub fn order<'a>(&self, items: &'a [T]) -> impl Iterator<Item = &'a T> + use<'a, T> {
        let start = self.start(items);
        let (wrapped, resumed) = items.split_at(start);
        iter::chain(resumed, wrapped)
    }

    fn start(&self, items: &[T]) -> usize {
        self.last
            .as_ref()
            .and_then(|last| items.iter().position(|item| item == last))
            .map_or(0, |i| i + 1)
    }
}

impl<T> Default for RoundRobin<T> {
    fn default() -> Self {
        Self { last: None }
    }
}
