use bevy::{
    prelude::*,
    text::{Text, TextStyle},
};

use crate::{
    presenter_state::PresenterGraphChanged, tracked_components::TrackedComponents,
    tracked_resources::TrackedResources, Cx, ViewHandle, ViewTuple,
};

use crate::node_span::NodeSpan;

use super::{
    presenter_state::PresenterStateChanged,
    view_children::ViewChildren,
    view_classes::{ClassNamesTuple, ViewClasses},
    view_insert::ViewInsert,
    view_styled::{StyleTuple, ViewStyled},
    view_with::ViewWith,
};

/// Passed to `build` and `raze` methods to give access to the world and the view entity.
pub struct ViewContext<'w> {
    pub(crate) world: &'w mut World,

    /// The entity which contains the PresenterState.
    pub(crate) entity: Entity,
}

impl<'w> ViewContext<'w> {
    pub(crate) fn new(world: &'w mut World, entity: Entity) -> Self {
        Self { world, entity }
    }

    /// Indicate that the shape of the display graph has changed.
    pub fn mark_changed_shape(&mut self) {
        self.world
            .entity_mut(self.entity)
            .insert(PresenterGraphChanged);
    }

    pub(crate) fn add_tracked_resource<T: Resource>(&mut self) {
        if let Some(mut tracked) = self.world.get_mut::<TrackedResources>(self.entity) {
            tracked.add_resource::<T>();
        } else {
            let mut tracked = TrackedResources::default();
            tracked.add_resource::<T>();
            self.world.entity_mut(self.entity).insert(tracked);
        }
    }

    pub(crate) fn add_tracked_component<C: Component>(&mut self, entity: Entity) {
        let id = self
            .world
            .component_id::<C>()
            .expect("Unregistered component type");
        if let Some(mut tracked) = self.world.get_mut::<TrackedComponents>(self.entity) {
            tracked.add_component(entity, id);
        } else {
            let mut tracked = TrackedComponents::default();
            tracked.add_component(entity, id);
            self.world.entity_mut(self.entity).insert(tracked);
        }
    }

    pub(crate) fn entity(&self, entity: Entity) -> EntityRef {
        self.world.entity(entity)
    }

    pub(crate) fn entity_mut(&mut self, entity: Entity) -> EntityWorldMut {
        self.world.entity_mut(entity)
    }
}

/// An object which generates one or more display nodes. Output of a presenter function
pub trait View: Send
where
    Self: Sized,
{
    /// The external state for this View.
    type State: Send;

    /// Return the span of UiNodes produced by this View.
    fn nodes(&self, vc: &ViewContext, state: &Self::State) -> NodeSpan;

    /// Construct and patch the tree of UiNodes produced by this view.
    /// This may also spawn child entities representing nested components.
    fn build(&self, vc: &mut ViewContext) -> Self::State;

    /// Update the internal state of this view, re-creating any UiNodes.
    fn update(&self, vc: &mut ViewContext, state: &mut Self::State);

    /// Attach child nodes to parents. This is typically called after generating/updating
    /// the display nodes (via build/rebuild), however it can also be called after rebuilding
    /// the display graph of nested presenters.
    fn assemble(&self, vc: &mut ViewContext, state: &mut Self::State) -> NodeSpan {
        self.nodes(vc, state)
    }

    /// Recursively despawn any child entities that were created as a result of calling `.build()`.
    /// This calls `.raze()` for any nested views within the current view state.
    fn raze(&self, vc: &mut ViewContext, state: &mut Self::State);

    /// Apply styles to this view.
    fn styled<S: StyleTuple>(self, styles: S) -> ViewStyled<Self> {
        ViewStyled::new(self, styles)
    }

    /// Set the class names for this View.
    fn class_names<S: ClassNamesTuple>(self, class_names: S) -> ViewClasses<Self> {
        ViewClasses::new(self, class_names)
    }

    /// Inserts a default instance of the specified component to the nodes generated by this view,
    /// if it's not already inserted.
    fn insert<C: Component + Clone>(self, component: C) -> ViewInsert<Self, C> {
        ViewInsert {
            inner: self,
            component,
        }
    }

    /// Sets up a callback which is called for each output UiNode generated by this `View`.
    /// Typically used to manipulate components on the entity. This is called each time the
    /// view is rebuilt.
    fn with<F: Fn(EntityWorldMut) -> () + Send>(self, callback: F) -> ViewWith<Self, F> {
        ViewWith {
            inner: self,
            callback,
            once: false,
        }
    }

    /// Sets up a callback which is called for each output UiNode generated by this `View`, but
    /// only when the node is first created. This should only be used in cases where you know
    /// that the closure won't change during rebuilds (that is, the set of captured values
    /// will always be the same once the `View` has been built.)
    fn once<F: Fn(EntityWorldMut) -> () + Send>(self, callback: F) -> ViewWith<Self, F> {
        ViewWith {
            inner: self,
            callback,
            once: true,
        }
    }

    /// Sets up a callback which is called for each output UiNode, but only when the node is first
    /// created.
    fn children<A: ViewTuple>(self, items: A) -> ViewChildren<Self, A> {
        ViewChildren { inner: self, items }
    }
}

/// View which renders nothing
impl View for () {
    type State = ();

    fn nodes(&self, _vc: &ViewContext, _state: &Self::State) -> NodeSpan {
        NodeSpan::Empty
    }

    fn build(&self, _vc: &mut ViewContext) -> Self::State {
        ()
    }

    fn update(&self, _vc: &mut ViewContext, _state: &mut Self::State) {}

    fn raze(&self, _vc: &mut ViewContext, _state: &mut Self::State) {}
}

/// View which renders a String
impl View for String {
    type State = Entity;

    fn nodes(&self, _vc: &ViewContext, state: &Self::State) -> NodeSpan {
        NodeSpan::Node(*state)
    }

    fn build(&self, vc: &mut ViewContext) -> Self::State {
        let id = vc
            .world
            .spawn((TextBundle {
                text: Text::from_section(self.clone(), TextStyle { ..default() }),
                // TextStyle {
                //     font_size: 40.0,
                //     color: Color::rgb(0.9, 0.9, 0.9),
                //     ..Default::default()
                // },
                // background_color: Color::rgb(0.65, 0.75, 0.65).into(),
                // border_color: Color::BLUE.into(),
                // focus_policy: FocusPolicy::Pass,
                ..default()
            },))
            .id();
        id
    }

    fn update(&self, vc: &mut ViewContext, state: &mut Self::State) {
        // If it's a single node and has a text component
        let nodes = self.nodes(vc, state);
        if let NodeSpan::Node(text_node) = nodes {
            if let Some(mut old_text) = vc.entity_mut(text_node).get_mut::<Text>() {
                // TODO: compare text for equality.
                old_text.sections.clear();
                old_text.sections.push(TextSection {
                    value: self.to_owned(),
                    style: TextStyle { ..default() },
                });
                return;
            }
        }

        // Despawn node and create new text node
        nodes.despawn(vc.world);
        vc.mark_changed_shape();
        *state = self.build(vc)
    }

    fn raze(&self, vc: &mut ViewContext, state: &mut Self::State) {
        let mut entt = vc.entity_mut(*state);
        entt.remove_parent();
        entt.despawn();
    }
}

/// View which renders a string slice.
impl View for &str {
    type State = Entity;

    fn nodes(&self, _vc: &ViewContext, state: &Self::State) -> NodeSpan {
        NodeSpan::Node(*state)
    }

    fn build(&self, vc: &mut ViewContext) -> Self::State {
        let id = vc
            .world
            .spawn((TextBundle {
                text: Text::from_section(self.to_string(), TextStyle { ..default() }),
                // TextStyle {
                //     font_size: 40.0,
                //     color: Color::rgb(0.9, 0.9, 0.9),
                //     ..Default::default()
                // },
                // background_color: Color::rgb(0.65, 0.75, 0.65).into(),
                // border_color: Color::BLUE.into(),
                // focus_policy: FocusPolicy::Pass,
                ..default()
            },))
            .id();
        id
    }

    fn update(&self, vc: &mut ViewContext, state: &mut Self::State) {
        // If it's a single node and has a text component
        let nodes = self.nodes(vc, state);
        if let NodeSpan::Node(text_node) = nodes {
            if let Some(mut old_text) = vc.entity_mut(text_node).get_mut::<Text>() {
                // TODO: compare text for equality.
                old_text.sections.clear();
                old_text.sections.push(TextSection {
                    value: self.to_string(),
                    style: TextStyle { ..default() },
                });
                return;
            }
        }

        // Despawn node and create new text node
        nodes.despawn(vc.world);
        vc.mark_changed_shape();
        *state = self.build(vc)
    }

    fn raze(&self, vc: &mut ViewContext, state: &mut Self::State) {
        let mut entt = vc.entity_mut(*state);
        entt.remove_parent();
        entt.despawn();
    }
}

/// View which renders a bare presenter with no arguments
impl<V: View + 'static, F: Fn(Cx<()>) -> V + Send + Copy + 'static> View for F {
    // State holds the PresenterState entity.
    type State = Entity;

    fn nodes(&self, vc: &ViewContext, state: &Self::State) -> NodeSpan {
        // get the handle from the PresenterState for this invocation.
        let entt = vc.entity(*state);
        let Some(ref handle) = entt.get::<ViewHandle>() else {
            return NodeSpan::Empty;
        };
        handle.inner.lock().unwrap().nodes()
    }

    fn build(&self, parent_ecx: &mut ViewContext) -> Self::State {
        let entity = parent_ecx
            .world
            .spawn(ViewHandle::new(*self, ()))
            .insert(PresenterStateChanged)
            .set_parent(parent_ecx.entity)
            .id();
        // Not calling build here: will be done asynchronously.
        entity
    }

    fn update(&self, _parent_ecx: &mut ViewContext, _state: &mut Self::State) {
        // Rebuild does nothing: it's up to the child to decide whether or not it wants to
        // rebuild. Since there are no props, we don't mark the child as modified.
    }

    fn raze(&self, vc: &mut ViewContext, state: &mut Self::State) {
        let mut entt = vc.entity_mut(*state);
        let Some(handle) = entt.get_mut::<ViewHandle>() else {
            return;
        };
        let inner = handle.inner.clone();
        // Raze the contents of the child ViewState.
        inner.lock().unwrap().raze(vc, *state);
        // Despawn the ViewHandle.
        vc.entity_mut(*state).remove_parent();
        vc.entity_mut(*state).despawn();
    }
}

/// Binds a presenter to properties and implements a view
#[doc(hidden)]
pub struct Bind<V: View, Props: Send + Clone, F: FnMut(Cx<Props>) -> V + Copy> {
    presenter: F,
    props: Props,
}

impl<V: View, Props: Send + Clone, F: FnMut(Cx<Props>) -> V + Copy> Bind<V, Props, F> {
    pub fn new(presenter: F, props: Props) -> Self {
        Self { presenter, props }
    }
}

impl<
        V: View + 'static,
        Props: Send + Clone + PartialEq + 'static,
        F: FnMut(Cx<Props>) -> V + Send + Copy + 'static,
    > View for Bind<V, Props, F>
{
    // State holds the PresenterState entity.
    type State = Entity;

    fn nodes(&self, vc: &ViewContext, state: &Self::State) -> NodeSpan {
        // get the handle from the PresenterState for this invocation.
        let entt = vc.entity(*state);
        let Some(ref handle) = entt.get::<ViewHandle>() else {
            return NodeSpan::Empty;
        };
        handle.inner.lock().unwrap().nodes()
    }

    fn build(&self, parent_ecx: &mut ViewContext) -> Self::State {
        let entity = parent_ecx
            .world
            .spawn(ViewHandle::new(self.presenter, self.props.clone()))
            .insert(PresenterStateChanged)
            .set_parent(parent_ecx.entity)
            .id();
        // Not calling build here: will be done asynchronously.
        entity
    }

    fn update(&self, vc: &mut ViewContext, state: &mut Self::State) {
        // get the handle from the current view state
        let mut entt = vc.entity_mut(*state);
        let Some(mut handle) = entt.get_mut::<ViewHandle>() else {
            return;
        };
        // Update child view properties.
        if handle.update_props(&self.props) {
            entt.insert(PresenterStateChanged);
        }
    }

    fn raze(&self, vc: &mut ViewContext, state: &mut Self::State) {
        let mut entt = vc.entity_mut(*state);
        let Some(handle) = entt.get_mut::<ViewHandle>() else {
            return;
        };
        let inner = handle.inner.clone();
        // Raze the contents of the child ViewState.
        inner.lock().unwrap().raze(vc, *state);
        // Despawn the ViewHandle.
        vc.entity_mut(*state).remove_parent();
        vc.entity_mut(*state).despawn();
    }
}

/// A trait that allows methods to be added to presenter function references.
pub trait PresenterFn<V: View, Props: Send + Clone, F: FnMut(Cx<Props>) -> V + Copy> {
    /// Used to invoke a presenter from within a presenter. This binds a set of properties
    /// to the child presenter, and constructs a new `ViewHandle`/`PresenterState`. The
    /// resulting is a `View` which references this handle.
    fn bind(self, props: Props) -> Bind<V, Props, F>;
}

impl<V: View, Props: Send + Clone, F: FnMut(Cx<Props>) -> V + Copy> PresenterFn<V, Props, F> for F {
    fn bind(self, props: Props) -> Bind<V, Props, Self> {
        Bind::new(self, props)
    }
}
