use crate::selection::Selected;
use crate::utils::entity::replace_with_empty;
use bevy::prelude::{Entity, World};
use phichain_chart::noise::NoiseArea;
use phichain_game::noise::NoiseAreaOrder;
use undo::Edit;

#[derive(Debug, Clone)]
pub struct CreateNoiseArea {
    pub area: NoiseArea,
    entity: Option<Entity>,
    order: Option<usize>,
}

impl CreateNoiseArea {
    pub fn new(area: NoiseArea) -> Self {
        Self {
            area,
            entity: None,
            order: None,
        }
    }
}

impl Edit for CreateNoiseArea {
    type Target = World;
    type Output = ();

    fn edit(&mut self, target: &mut Self::Target) {
        let order = *self.order.get_or_insert_with(|| {
            target
                .query::<&NoiseAreaOrder>()
                .iter(target)
                .map(|order| order.0)
                .max()
                .map_or(0, |order| order + 1)
        });
        let selected = target
            .query_filtered::<Entity, bevy::prelude::With<Selected>>()
            .iter(target)
            .collect::<Vec<_>>();
        for entity in selected {
            target.entity_mut(entity).remove::<Selected>();
        }
        if let Some(entity) = self.entity {
            target
                .entity_mut(entity)
                .insert((self.area.clone(), NoiseAreaOrder(order), Selected));
        } else {
            self.entity = Some(
                target
                    .spawn((self.area.clone(), NoiseAreaOrder(order), Selected))
                    .id(),
            );
        }
    }

    fn undo(&mut self, target: &mut Self::Target) {
        if let Some(entity) = self.entity {
            replace_with_empty(target, entity);
        }
    }
}

#[derive(Debug, Clone)]
pub struct RemoveNoiseArea {
    pub entity: Entity,
    removed: Option<(NoiseArea, NoiseAreaOrder)>,
}

impl RemoveNoiseArea {
    pub fn new(entity: Entity) -> Self {
        Self {
            entity,
            removed: None,
        }
    }
}

impl Edit for RemoveNoiseArea {
    type Target = World;
    type Output = ();

    fn edit(&mut self, target: &mut Self::Target) {
        let area = target.get::<NoiseArea>(self.entity).cloned();
        let order = target.get::<NoiseAreaOrder>(self.entity).copied();
        if let (Some(area), Some(order)) = (area, order) {
            self.removed = Some((area, order));
            replace_with_empty(target, self.entity);
        }
    }

    fn undo(&mut self, target: &mut Self::Target) {
        if let Some((area, order)) = self.removed.clone() {
            target.entity_mut(self.entity).insert((area, order));
        }
    }
}

#[derive(Debug, Clone)]
pub struct EditNoiseArea {
    pub entity: Entity,
    pub from: NoiseArea,
    pub to: NoiseArea,
}

impl Edit for EditNoiseArea {
    type Target = World;
    type Output = ();

    fn edit(&mut self, target: &mut Self::Target) {
        if let Some(mut area) = target.get_mut::<NoiseArea>(self.entity) {
            *area = self.to.clone();
        }
    }

    fn undo(&mut self, target: &mut Self::Target) {
        if let Some(mut area) = target.get_mut::<NoiseArea>(self.entity) {
            *area = self.from.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editing::command::EditorCommand;
    use undo::History;

    #[test]
    fn remove_undo_preserves_entity_for_earlier_edits() {
        let mut world = World::new();
        let from = NoiseArea::default();
        let mut to = from.clone();
        to.is_subtract = true;
        let entity = world.spawn((from.clone(), NoiseAreaOrder(0))).id();
        let mut history = History::new();

        history.edit(
            &mut world,
            EditorCommand::EditNoiseArea(EditNoiseArea {
                entity,
                from: from.clone(),
                to: to.clone(),
            }),
        );
        history.edit(
            &mut world,
            EditorCommand::RemoveNoiseArea(RemoveNoiseArea::new(entity)),
        );
        history.undo(&mut world);
        assert!(world.get::<NoiseArea>(entity).unwrap().is_subtract);
        history.undo(&mut world);
        assert!(!world.get::<NoiseArea>(entity).unwrap().is_subtract);
    }
}
