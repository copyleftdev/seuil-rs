//! Iterator over array/range members.

use super::Value;

pub struct MemberIterator<'a> {
    value: &'a Value<'a>,
    front: usize,
    back: usize,
    back_done: bool,
}

impl<'a> MemberIterator<'a> {
    pub fn new(value: &'a Value<'a>) -> Self {
        Self {
            value,
            front: 0,
            back: value.len().saturating_sub(1),
            back_done: false,
        }
    }
}

impl<'a> Iterator for MemberIterator<'a> {
    type Item = &'a Value<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.front < self.value.len() {
            let result = self.value.get_member(self.front);
            self.front += 1;
            result
        } else {
            None
        }
    }
}

impl<'a> DoubleEndedIterator for MemberIterator<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.back_done {
            return None;
        }

        let result = self.value.get_member(self.back);

        if self.back == 0 {
            self.back_done = true;
            return result;
        }

        self.back -= 1;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bumpalo::Bump;

    #[test]
    fn forward_iteration() {
        let arena = Bump::new();
        let range = Value::range(&arena, 1, 5);
        let values: Vec<f64> = MemberIterator::new(range).map(|v| v.as_f64()).collect();
        assert_eq!(values, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn backward_iteration() {
        let arena = Bump::new();
        let range = Value::range(&arena, 1, 5);
        let values: Vec<f64> = MemberIterator::new(range)
            .rev()
            .map(|v| v.as_f64())
            .collect();
        assert_eq!(values, vec![5.0, 4.0, 3.0, 2.0, 1.0]);
    }
}
