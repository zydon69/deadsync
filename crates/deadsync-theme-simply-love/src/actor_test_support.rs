//! Compare rendered values while allowing the intentional text storage migration.
use deadlib_present::actors::{Actor, TextContent};

pub(crate) fn canonical(actors: &[Actor]) -> String {
    fn normalize(actor: &mut Actor) {
        match actor {
            Actor::Text { content, .. }
                if matches!(
                    content,
                    TextContent::Static(_) | TextContent::Owned(_) | TextContent::Shared(_)
                ) =>
            {
                *content = TextContent::Owned(content.as_str().to_owned());
            }
            Actor::Frame { children, .. } => children.iter_mut().for_each(normalize),
            Actor::SharedFrame { children, .. } => {
                std::sync::Arc::make_mut(children)
                    .iter_mut()
                    .for_each(normalize);
            }
            _ => {}
        }
    }
    let mut actors = actors.to_vec();
    actors.iter_mut().for_each(normalize);
    format!("{actors:?}")
}
