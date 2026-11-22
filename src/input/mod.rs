//! Input bindings.
//!
//! Gameplay systems never hard-code keys; they read [`Bindings`], which the
//! settings screen will make rebindable. Defaults follow the first-person
//! spec: WASD, Shift sprint, Space jump, E pick/interact, Tab basket, K skill
//! tree, J journal, M map, C collection, Esc pause.

use bevy::prelude::*;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Bindings>();
    }
}

/// Action → key bindings. Several keys may map to one action.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Bindings {
    pub forward: Vec<KeyCode>,
    pub back: Vec<KeyCode>,
    pub left: Vec<KeyCode>,
    pub right: Vec<KeyCode>,
    pub sprint: Vec<KeyCode>,
    pub jump: Vec<KeyCode>,
    pub interact: Vec<KeyCode>,
    pub inventory: Vec<KeyCode>,
    /// The skill tree.
    pub skills: Vec<KeyCode>,
    /// The journal (achievements, statistics, progress).
    pub journal: Vec<KeyCode>,
    /// The world map.
    pub map: Vec<KeyCode>,
    /// The herbarium (collection).
    pub collection: Vec<KeyCode>,
    /// Tool bar slots 1–4 (hands/scissors, basket, rake, cart).
    pub tool_slots: [KeyCode; 4],
    pub pause: Vec<KeyCode>,
    pub confirm: Vec<KeyCode>,
    pub toggle_debug: Vec<KeyCode>,
}

impl Default for Bindings {
    fn default() -> Self {
        use KeyCode::*;
        Self {
            forward: vec![KeyW, ArrowUp],
            back: vec![KeyS, ArrowDown],
            left: vec![KeyA, ArrowLeft],
            right: vec![KeyD, ArrowRight],
            sprint: vec![ShiftLeft, ShiftRight],
            jump: vec![Space],
            interact: vec![KeyE],
            inventory: vec![Tab],
            skills: vec![KeyK],
            journal: vec![KeyJ],
            map: vec![KeyM],
            collection: vec![KeyC],
            tool_slots: [Digit1, Digit2, Digit3, Digit4],
            pause: vec![Escape],
            confirm: vec![Enter, NumpadEnter],
            toggle_debug: vec![F3],
        }
    }
}

impl Bindings {
    pub fn pressed(keys: &ButtonInput<KeyCode>, codes: &[KeyCode]) -> bool {
        keys.any_pressed(codes.iter().copied())
    }

    pub fn just_pressed(keys: &ButtonInput<KeyCode>, codes: &[KeyCode]) -> bool {
        keys.any_just_pressed(codes.iter().copied())
    }

    /// Movement keys as a local vector: x = right, y = forward.
    pub fn move_axis(&self, keys: &ButtonInput<KeyCode>) -> Vec2 {
        let axis = |pos: &[KeyCode], neg: &[KeyCode]| {
            Self::pressed(keys, pos) as i8 as f32 - Self::pressed(keys, neg) as i8 as f32
        };
        Vec2::new(
            axis(&self.right, &self.left),
            axis(&self.forward, &self.back),
        )
    }
}

/// Actions the player can rebind (their primary key; movement keeps the
/// arrow keys as a second choice). Esc, Enter, F3, the arrows and 1-4 stay
/// fixed so menus always work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Forward,
    Back,
    Left,
    Right,
    Sprint,
    Jump,
    Interact,
    Basket,
    Skills,
    Journal,
    Map,
    Collection,
}

impl Action {
    pub const ALL: [Action; 12] = [
        Action::Forward,
        Action::Back,
        Action::Left,
        Action::Right,
        Action::Sprint,
        Action::Jump,
        Action::Interact,
        Action::Basket,
        Action::Skills,
        Action::Journal,
        Action::Map,
        Action::Collection,
    ];

    /// Stable id used in `settings.ron`.
    pub fn id(self) -> &'static str {
        match self {
            Action::Forward => "forward",
            Action::Back => "back",
            Action::Left => "left",
            Action::Right => "right",
            Action::Sprint => "sprint",
            Action::Jump => "jump",
            Action::Interact => "interact",
            Action::Basket => "basket",
            Action::Skills => "skills",
            Action::Journal => "journal",
            Action::Map => "map",
            Action::Collection => "collection",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Action::Forward => "Walk forward",
            Action::Back => "Walk back",
            Action::Left => "Step left",
            Action::Right => "Step right",
            Action::Sprint => "Sprint",
            Action::Jump => "Jump",
            Action::Interact => "Pick / use",
            Action::Basket => "Basket",
            Action::Skills => "Skill tree",
            Action::Journal => "Journal",
            Action::Map => "World map",
            Action::Collection => "Herbarium",
        }
    }
}

/// Keys that can be bound, with the names shown and saved.
const KEY_NAMES: &[(KeyCode, &str)] = {
    use KeyCode::*;
    &[
        (KeyA, "A"),
        (KeyB, "B"),
        (KeyC, "C"),
        (KeyD, "D"),
        (KeyE, "E"),
        (KeyF, "F"),
        (KeyG, "G"),
        (KeyH, "H"),
        (KeyI, "I"),
        (KeyJ, "J"),
        (KeyK, "K"),
        (KeyL, "L"),
        (KeyM, "M"),
        (KeyN, "N"),
        (KeyO, "O"),
        (KeyP, "P"),
        (KeyQ, "Q"),
        (KeyR, "R"),
        (KeyS, "S"),
        (KeyT, "T"),
        (KeyU, "U"),
        (KeyV, "V"),
        (KeyW, "W"),
        (KeyX, "X"),
        (KeyY, "Y"),
        (KeyZ, "Z"),
        (Digit5, "5"),
        (Digit6, "6"),
        (Digit7, "7"),
        (Digit8, "8"),
        (Digit9, "9"),
        (Digit0, "0"),
        (Space, "Space"),
        (Tab, "Tab"),
        (ShiftLeft, "Left Shift"),
        (ShiftRight, "Right Shift"),
        (ControlLeft, "Left Ctrl"),
        (ControlRight, "Right Ctrl"),
        (AltLeft, "Left Alt"),
        (AltRight, "Right Alt"),
        (CapsLock, "Caps Lock"),
        (Backspace, "Backspace"),
        (Backquote, "`"),
        (Minus, "-"),
        (Equal, "="),
        (BracketLeft, "["),
        (BracketRight, "]"),
        (Semicolon, ";"),
        (Quote, "'"),
        (Comma, ","),
        (Period, "."),
        (Slash, "/"),
        (Backslash, "\\"),
        (F1, "F1"),
        (F2, "F2"),
        (F4, "F4"),
        (F5, "F5"),
        (F6, "F6"),
        (F7, "F7"),
        (F8, "F8"),
        (F9, "F9"),
        (F10, "F10"),
        (F11, "F11"),
        (F12, "F12"),
    ]
};

/// The name of a bindable key (`None` for keys that can't be bound).
pub fn key_name(key: KeyCode) -> Option<&'static str> {
    KEY_NAMES.iter().find(|(k, _)| *k == key).map(|(_, n)| *n)
}

pub fn key_from_name(name: &str) -> Option<KeyCode> {
    KEY_NAMES.iter().find(|(_, n)| *n == name).map(|(k, _)| *k)
}

/// Primary-key overrides as saved in the settings: action id → key name.
pub type KeyOverrides = std::collections::BTreeMap<String, String>;

impl Bindings {
    fn keys_mut(&mut self, action: Action) -> &mut Vec<KeyCode> {
        match action {
            Action::Forward => &mut self.forward,
            Action::Back => &mut self.back,
            Action::Left => &mut self.left,
            Action::Right => &mut self.right,
            Action::Sprint => &mut self.sprint,
            Action::Jump => &mut self.jump,
            Action::Interact => &mut self.interact,
            Action::Basket => &mut self.inventory,
            Action::Skills => &mut self.skills,
            Action::Journal => &mut self.journal,
            Action::Map => &mut self.map,
            Action::Collection => &mut self.collection,
        }
    }

    /// The name of an action's key, for hints ("K", "Tab").
    pub fn label(&self, action: Action) -> &'static str {
        key_name(self.primary(action)).unwrap_or("?")
    }

    /// The key shown for an action.
    pub fn primary(&self, action: Action) -> KeyCode {
        let mut b = self.clone();
        b.keys_mut(action)[0]
    }

    /// Defaults with the saved overrides applied (unknown entries ignored).
    pub fn from_overrides(overrides: &KeyOverrides) -> Self {
        let mut b = Self::default();
        for action in Action::ALL {
            if let Some(key) = overrides.get(action.id()).and_then(|n| key_from_name(n)) {
                let keys = b.keys_mut(action);
                // Shift binds both sides by default; an override is one key.
                keys.retain(|k| key_name(*k).is_none());
                keys.insert(0, key);
            }
        }
        b
    }
}

/// Why a key can't be bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebindError {
    /// Menus need it (Esc, Enter, F3, arrows, 1-4).
    Reserved,
}

/// Bind `key` as the primary key of `action`. If another action already
/// uses it, the two swap keys (so nothing is ever left unbound). Returns
/// the action that swapped, if any.
pub fn rebind(
    overrides: &mut KeyOverrides,
    action: Action,
    key: KeyCode,
) -> Result<Option<Action>, RebindError> {
    let Some(name) = key_name(key) else {
        return Err(RebindError::Reserved);
    };
    let current = Bindings::from_overrides(overrides);
    let old = current.primary(action);
    let other = Action::ALL
        .into_iter()
        .find(|&a| a != action && Bindings::pressed_by(&current, a, key));
    overrides.insert(action.id().to_string(), name.to_string());
    if let Some(other) = other {
        if let Some(old_name) = key_name(old) {
            overrides.insert(other.id().to_string(), old_name.to_string());
        }
    }
    Ok(other)
}

impl Bindings {
    fn pressed_by(b: &Bindings, action: Action, key: KeyCode) -> bool {
        let mut b = b.clone();
        b.keys_mut(action).contains(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebinding_swaps_conflicts_and_refuses_reserved_keys() {
        let mut o = KeyOverrides::new();
        // M is the map; give it to the journal: they swap.
        assert_eq!(
            rebind(&mut o, Action::Journal, KeyCode::KeyM),
            Ok(Some(Action::Map))
        );
        let b = Bindings::from_overrides(&o);
        assert_eq!(b.primary(Action::Journal), KeyCode::KeyM);
        assert_eq!(b.primary(Action::Map), KeyCode::KeyJ);
        // Movement keeps its arrow keys.
        assert_eq!(rebind(&mut o, Action::Forward, KeyCode::KeyI), Ok(None));
        let b = Bindings::from_overrides(&o);
        assert_eq!(b.forward, vec![KeyCode::KeyI, KeyCode::ArrowUp]);
        // Shift: an override replaces both sides.
        rebind(&mut o, Action::Sprint, KeyCode::ControlLeft).unwrap();
        assert_eq!(
            Bindings::from_overrides(&o).sprint,
            vec![KeyCode::ControlLeft]
        );
        for key in [
            KeyCode::Escape,
            KeyCode::Enter,
            KeyCode::F3,
            KeyCode::ArrowLeft,
            KeyCode::Digit1,
        ] {
            assert_eq!(
                rebind(&mut o, Action::Jump, key),
                Err(RebindError::Reserved)
            );
        }
        // Every action always has a key and no two share a primary.
        let b = Bindings::from_overrides(&o);
        let mut seen = std::collections::HashSet::new();
        for a in Action::ALL {
            assert!(seen.insert(b.primary(a)), "{a:?}");
        }
    }

    #[test]
    fn names_round_trip_and_bad_overrides_are_ignored() {
        for (k, n) in KEY_NAMES {
            assert_eq!(key_from_name(n), Some(*k));
            assert_eq!(key_name(*k), Some(*n));
        }
        let mut o = KeyOverrides::new();
        o.insert("map".into(), "Nonsense".into());
        o.insert("teleport".into(), "T".into());
        assert_eq!(Bindings::from_overrides(&o).map, Bindings::default().map);
    }
}
