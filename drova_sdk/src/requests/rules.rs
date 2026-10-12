use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

use url::Url;

use super::{RequestInfo, Source};
use crate::sites::{UrlFacts, UrlIndex, UrlMatcher};

type Modifier<'a> = Box<dyn Fn(&RequestInfo<'_>, &mut dyn Any) + Send + Sync + 'a>;

struct Rule<'a> {
    matcher: UrlMatcher,
    modify: Modifier<'a>,
}

#[derive(Default)]
struct Bucket<'a> {
    rules: Vec<Rule<'a>>,
    index: UrlIndex,
}

#[derive(Default)]
pub(crate) struct RequestRules<'a> {
    buckets: HashMap<TypeId, Bucket<'a>>,
}

impl<'a> RequestRules<'a> {
    pub fn has<R: 'static>(&self) -> bool {
        self.buckets.contains_key(&TypeId::of::<R>())
    }
    pub fn insert<R: 'static>(
        &mut self,
        matcher: UrlMatcher,
        modify: impl Fn(&RequestInfo<'_>, R) -> R + Send + Sync + 'a,
    ) {
        let erased = move |info: &RequestInfo<'_>, request: &mut dyn Any| {
            let slot = request
                .downcast_mut::<Option<R>>()
                .expect("request bucket type");
            *slot = Some(modify(info, slot.take().expect("request value")));
        };
        self.buckets
            .entry(TypeId::of::<R>())
            .or_default()
            .rules
            .push(Rule {
                matcher,
                modify: Box::new(erased),
            });
    }

    pub fn build(&mut self) {
        for bucket in self.buckets.values_mut() {
            bucket.index = UrlIndex::build(bucket.rules.iter().map(|rule| &rule.matcher));
        }
    }

    pub fn apply<R: 'static>(&self, url: &Url, source: Option<&Source<'_>>, request: R) -> R {
        let Some(bucket) = self.buckets.get(&TypeId::of::<R>()) else {
            return request;
        };
        let facts = UrlFacts::new(url);
        let info = RequestInfo { url, source };
        let mut slot = Some(request);
        for id in bucket.index.candidates(&facts) {
            let rule = &bucket.rules[id];
            if rule.matcher.matches_facts(&facts) {
                (rule.modify)(&info, &mut slot);
            }
        }
        slot.expect("request value after modifiers")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_rule_bucket_preserves_order_and_skips_unrelated_hosts() {
        let mut rules = RequestRules::default();
        rules.insert::<Vec<usize>>(UrlMatcher::new(), |_, mut r| {
            r.push(0);
            r
        });
        for id in 1..10_000 {
            rules.insert::<Vec<usize>>(
                UrlMatcher::new().hosts([format!("host{id}.org")]),
                |_, _| panic!("unrelated callback"),
            );
        }
        rules.insert::<Vec<usize>>(UrlMatcher::new().hosts(["*.example.org"]), |_, mut r| {
            r.push(1);
            r
        });
        rules.insert::<Vec<usize>>(
            UrlMatcher::new().hosts(["a.example.org", "*.org"]),
            |_, mut r| {
                r.push(2);
                r
            },
        );
        rules.build();
        let url = Url::parse("https://a.example.org/").unwrap();
        assert_eq!(rules.apply(&url, None, Vec::<usize>::new()), [0, 1, 2]);
    }

    #[test]
    #[ignore = "release timing benchmark"]
    fn rules_benchmark() {
        use std::{hint::black_box, time::Instant};
        let url = Url::parse("https://target.example.org/").unwrap();
        for count in [0, 1, 100, 10_000] {
            let mut rules = RequestRules::default();
            for id in 0..count {
                rules.insert::<usize>(
                    UrlMatcher::new().hosts([format!("host{id}.org")]),
                    |_, r| r + 1,
                );
            }
            rules.insert::<usize>(UrlMatcher::new().hosts(["target.example.org"]), |_, r| {
                r + 1
            });
            rules.build();
            let start = Instant::now();
            for _ in 0..100_000 {
                black_box(rules.apply(black_box(&url), None, 0usize));
            }
            println!(
                "{count} unrelated rules, 100000 lookups: {:?}",
                start.elapsed()
            );
        }
    }
}
