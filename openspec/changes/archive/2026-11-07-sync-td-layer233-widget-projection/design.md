# Design

## Decision record: project historical bot web widgets

Source: `iamxvbaba/td` `main@cddaf9381` (merge of `roon9e/feat/legacy-widget-media@840a3d46e`,
"feat(protocol): project historical bot web widgets to messageMediaUnsupported").

### Policy

`policy.json` grows from 305 to 313 entries. The eight new entries are the `messageMediaWidget`
`new-only` obligations (layers 225–232, canonical-to-profile), resolved:

```
action: project
hook:   layerProjectLegacyWidgetMedia
```

All 305 prior entries are unchanged.

### Behavior

When a canonical `messageMedia` value is the `messageMediaWidget` member and the target profile
predates Layer 233, the encoder substitutes `messageMediaUnsupported#9f84f49e` rather than failing
closed. At Layer 233 the widget is encoded unchanged. This is the first **family-level** project
hook (previous project hooks were field-shaped): it maps a whole class value to another canonical
constructor.

### Generator

`emit_profile` builds a `canonical constructor qname -> function` map from the new-only `project`
obligations and consults it in the class/object `encode_<class>_profile` dispatcher. When the
route is absent (the member has no wire identity in the profile), it calls the hook and re-enters
the dispatcher with the substitute, instead of returning `Unrepresentable`. The substitute is a
known canonical member, so no runtime dynamic dispatch is needed.

### Hook

`rtd_layerhooks::widget::project_legacy_widget_media(layer, &MessageMedia) -> MessageMedia`:
before 233 and for a widget member it returns `MessageMedia::MessageMediaUnsupported`; otherwise
it returns the value unchanged. Registered in the shared registry with a new `family_project`
shape.

### Verification

- `rtd-tlprofile` runtime test `widget_media_projects_to_unsupported_in_historical_profiles`: a
  frozen widget media round-trips through every layer 225–232 as `MessageMediaUnsupported`, and
  stays a widget at 233.
- Hook unit tests; `rtd-codegen generate`/`verify` accept the 313-entry policy; full workspace
  gate green.
