#!/usr/bin/env python3
"""Offline consistency verification of all Cargo manifest refs and Rust imports."""
from pathlib import Path
import json, re, tomllib, sys
root=Path(__file__).resolve().parent.parent
rename=json.loads((root/'RENAME_MAP.json').read_text())['package_names']
workspace=tomllib.loads((root/'Cargo.toml').read_text())['workspace']
errors=[]
packages={}
for group in ('projects','macros','ui','media','web','graphics','apps','tools','core'):
    for manifest in sorted((root/group).glob('*/Cargo.toml')):
        obj=tomllib.loads(manifest.read_text())
        name=obj['package']['name']
        if name!=manifest.parent.name:errors.append(f'folder mismatch: {name} vs {manifest.parent}')
        if name in packages:errors.append(f'duplicate {name}')
        if not re.fullmatch(r'[a-z][a-z0-9]*(?:-[a-z0-9]+)*',name):errors.append(f'non kebab-case: {name}')
        packages[name]=(manifest.parent,obj)
if len(packages)!=43:errors.append(f'expected 43 Cargo packages, got {len(packages)}')
for member in workspace.get('default-members',[]):
    if (root/member/'Cargo.toml').is_file() is False:errors.append(f'bad default member {member}')
for name,(base,obj) in packages.items():
    deps={}
    def walk(d):
        if not isinstance(d,dict):return
        for k,v in d.items():
            if k in ('dependencies','build-dependencies','dev-dependencies'):
                for dep,spec in v.items():
                    deps[dep]=spec
                    if isinstance(spec,dict):
                        if 'path' in spec:
                            path=base/spec['path']/'Cargo.toml'
                            if not path.is_file():errors.append(f'{name}: invalid path {dep} => {spec["path"]}')
                            else:
                                actual=tomllib.loads(path.read_text())['package']['name']
                                if actual!=spec.get('package',dep):errors.append(f'{name}: dependency {dep} package mismatch: {actual}')
                        if spec.get('workspace')==True and dep not in workspace.get('dependencies',{}):
                            errors.append(f'{name}: unresolved workspace dependency: {dep}')
            elif k!='metadata':walk(v)
    walk(obj)
    for feature,entries in obj.get('features',{}).items():
        for item in entries:
            if item.startswith('dep:') and item[4:] not in deps: errors.append(f'{name}: stale feature {item}')
            elif '/' in item and item.split('/')[0].rstrip('?') not in deps:errors.append(f'{name}: stale feature {item}')
    for file in base.rglob('*.rs'):
        rel=file.relative_to(base).as_posix()
        if rel.startswith('overrides/'): continue  # patch payloads are not Rust compilation units
        if name=='mata-win' and rel.startswith('src/widgets/'): continue
        if rel.startswith('examples/') and obj['package'].get('autoexamples') is False:
            allowed={ex.get('path','examples/'+ex['name']+'.rs') for ex in obj.get('example',[])}
            if rel not in allowed: continue
        text=file.read_text(encoding='utf-8',errors='replace')
        for old in rename:
            if re.search(r'(?<![A-Za-z0-9_])'+re.escape(old)+r'(?![A-Za-z0-9_])',text):
                errors.append(f'{file.relative_to(root)}: stale import/name {old}')
        for imported in set(re.findall(r'\b(?:use|extern\s+crate)\s+(?:r#)?([A-Za-z][A-Za-z0-9_]*)\b',text)):
            target=next((dep for dep in packages if dep.replace('-','_')==imported),None)
            if target and target!=name and target not in deps:
                errors.append(f'{file.relative_to(root)}: missing direct dependency for use {imported}')
for manifest in root.rglob('Cargo.toml'):
    text=manifest.read_text()
    for old in rename:
        if re.search(r'(?<![A-Za-z0-9_])'+re.escape(old)+r'(?![A-Za-z0-9_])',text):errors.append(f'{manifest}: old package name {old}')
removed={p for group in json.loads((root/'REMOVED_CRATES.json').read_text()).values() for p in group}
for p in removed:
    if (root/p/'Cargo.toml').exists():errors.append(f'removed crate returned: {p}')

# MIVL's native renderer is an unconditional public API, not a feature switch.
mivl_cargo=(root/'ui/mivl/Cargo.toml').read_text()
mivl_src=(root/'ui/mivl/src/lib.rs').read_text()
ui_events=(root/'ui/mata-ui/src/widgets/events.rs').read_text()
ui_mod=(root/'ui/mata-ui/src/widgets/mod.rs').read_text()
ui_window=(root/'ui/mata-ui/src/widgets/window.rs').read_text()
if 'native-ui' in mivl_cargo:errors.append('mivl: legacy native-ui Cargo feature still exists')
for dep in ('mata-ui','mata-win'):
    if not isinstance(packages['mivl'][1]['dependencies'].get(dep),dict):
        errors.append(f'mivl: {dep} dependency not declared')
    elif packages['mivl'][1]['dependencies'][dep].get('optional'):
        errors.append(f'mivl: {dep} is incorrectly optional')
if '#[cfg(feature = "native-ui")]' in mivl_src or 'pub mod native;' not in mivl_src:
    errors.append('mivl: native module is still gated or missing')
for symbol in ('pub enum TouchPhase', 'PointerMoved(', 'PointerPressed(', 'PointerReleased(', 'Scrolled {', 'Touched {'):
    if symbol not in ui_events:errors.append(f'mata-ui: missing widget input declaration: {symbol}')
if 'WidgetId, TouchPhase' not in ui_mod:errors.append('mata-ui: TouchPhase is not exported from widgets')
# Enum variants must be unique; duplicates can also break derive-generated matches.
widget_enum = ui_events.split('pub enum WidgetEvent {', 1)[1].split('\n}', 1)[0]
widget_variants = re.findall(r'^\s*([A-Za-z_][A-Za-z0-9_]*)\s*(?:\(|\{|,)', widget_enum, re.M)
for variant in sorted(set(widget_variants)):
    if widget_variants.count(variant) != 1:
        errors.append(f'mata-ui: WidgetEvent variant {variant} declared {widget_variants.count(variant)} times')
if widget_variants.count('KeyReleased') != 1:
    errors.append('mata-ui: WidgetEvent must declare exactly one KeyReleased variant')
for expected in ('poll_touches_into', 'WidgetEvent::PointerMoved', 'WidgetEvent::PointerPressed', 'WidgetEvent::PointerReleased', 'WidgetEvent::Scrolled', 'WidgetEvent::Touched'):
    if expected not in ui_window:errors.append(f'mata-ui: missing native forwarding: {expected}')
# Cross-platform DocEdit core must be independent of all third-party packages.
core_manifest=packages['mata-doc-edit'][1]
if core_manifest.get('dependencies') or any('dependencies' in key for key in core_manifest if key=='target'):
    errors.append('mata-doc-edit: only the Rust standard library is allowed')
if (root/'ui/mata-ui/src/widgets/richedit.rs').exists():errors.append('Old RichEdit module still present')
if 'richedit' in packages['mata-ui'][1].get('features',{}):errors.append('Old rich-edit feature is still declared')
if 'RichEditor' in (root/'apps/mata-document/src/main.rs').read_text():errors.append('App still imports RichEditor')
if 'compile_error!' in (root/'apps/mata-document/src/main.rs').read_text():errors.append('App still excludes non-Windows targets')
# Cross-platform DocEdit core must be independent of all third-party packages.
core_manifest=packages['mata-doc-edit'][1]
if core_manifest.get('dependencies') or any('dependencies' in key for key in core_manifest if key=='target'):
    errors.append('mata-doc-edit: only the Rust standard library is allowed')
if (root/'ui/mata-ui/src/widgets/richedit.rs').exists():errors.append('Old RichEdit module still present')
if 'richedit' in packages['mata-ui'][1].get('features',{}):errors.append('Old rich-edit feature is still declared')
if 'RichEditor' in (root/'apps/mata-document/src/main.rs').read_text():errors.append('App still imports RichEditor')
if 'compile_error!' in (root/'apps/mata-document/src/main.rs').read_text():errors.append('App still excludes non-Windows targets')
for name in ('mata-qml','mata-qml-macros'):
    if name in packages:errors.append(f'obsolete package still included: {name}')

# Build scripts must declare build-time crates in [build-dependencies].
doc_manifest=packages['mata-document'][1]
doc_build=(root/'apps/mata-document/build.rs')
if not doc_build.is_file():
    errors.append('mata-document: build.rs missing')
elif 'mivl_build::compile(' not in doc_build.read_text():
    errors.append('mata-document: build.rs does not compile the MIVL ribbon')
if 'mivl-build' not in doc_manifest.get('build-dependencies',{}):
    errors.append('mata-document: missing mivl-build [build-dependencies]')
if not (root/'apps/mata-document/ui/document_ui.mivl').is_file():
    errors.append('mata-document: MIVL ribbon source missing')
if 'pub fn compile(' not in (root/'tools/mivl-build/src/lib.rs').read_text():
    errors.append('mivl-build: expected public compile() function is missing')

# Button content uses the existing Image/Text/Group widget types.
ui_content=(root/'ui/mata-ui/src/widgets/content.rs').read_text()
ui_controls=(root/'ui/mata-ui/src/widgets/controls.rs').read_text()
ui_image=(root/'ui/mata-ui/src/widgets/catalog/image.rs').read_text()
mivl_native=(root/'ui/mivl/src/native.rs').read_text()
mivl_runtime=(root/'ui/mivl/src/runtime.rs').read_text()
for title, present in {
    'mata-ui: Image not convertible to child': 'impl From<Image> for WidgetContent' in ui_content,
    'mata-ui: nested Image not painted': 'WidgetContent::Image(v) =>' in ui_content,
    'mata-ui: Button cannot accept nested content': 'child: impl Into<WidgetContent>' in ui_controls,
    'mata-ui: Button#set_child missing': 'pub fn set_child(' in ui_controls,
    'mata-ui: Image requested size missing': 'pub fn set_size(&self, width: u32, height: u32)' in ui_image,
    'mata-ui: Image dimensions missing': 'pub fn dimensions(&self)' in ui_image,
    'mivl: source property not defined': '"source" if kind == "Image"' in mivl_runtime,
    'mivl: child Image renderer missing': '"Image" =>' in mivl_native,
    'mivl: nested content not captured by Button': 'within_button(&self.engine,i)' in mivl_native,
    'mivl: Image subtree must update on property changes': 'button_signature(&self.engine,i)' in mivl_native,
    'mivl: image feature not enabled': '"image"' in (root/'ui/mivl/Cargo.toml').read_text(),
    'mata-ui: updated example missing': (root/'ui/mata-ui/examples/icon_buttons.rs').is_file(),
    'mivl: icon sample image missing': (root/'ui/mivl/examples/assets/save.png').is_file(),
}.items():
    if not present: errors.append(title)
for title,old in {
    'mata-ui: legacy icon source still exists': (root/'ui/mata-ui/src/widgets/icon.rs').is_file(),
    'mata-ui: legacy icon API remains': 'pub fn new_icon(' in ui_controls or 'pub fn set_icon(' in ui_controls,
    'mivl: obsolete Icon property remains': '"icon" if kind == "Button"' in mivl_runtime,
    'mivl: obsolete iconSize property remains': '"iconSize" if kind == "Button"' in mivl_runtime,
}.items():
    if old: errors.append(title)


# The nested Mata Document ribbon must resolve to BOTH embedded palette variants.
icon_assets=root/'apps/mata-document/src/icon_assets.rs'
icons_root=root/'apps/mata-document/assets/icons'
ribbon=(root/'apps/mata-document/ui/document_ui.mivl').read_text()
source_names=set(re.findall(r'Image\s*\{\s*source:\s*"assets/icons/([^"/]+\.png)"', ribbon))
if not source_names:errors.append('mata-document: nested ribbon Image sources not found')
registry=icon_assets.read_text()
for source in source_names:
    for palette in ('light','dark'):
        path=icons_root/palette/source
        if not path.is_file():errors.append(f'mata-document: missing {palette} icon {source}')
        if f'assets/icons/{palette}/{source}' not in registry:errors.append(f'mata-document: {palette} icon not embedded: {source}')
if not ('pub enum IconTheme' in registry and 'pub fn bytes_for_theme(' in registry):
    errors.append('mata-document: themed icon resolver missing')
doc_main=(root/'apps/mata-document/src/main.rs').read_text()
detector_file=root/'apps/mata-document/src/theme_detection.rs'
if not detector_file.is_file():
    errors.append('mata-document: native theme detector missing')
else:
    detector=detector_file.read_text()
    for symbol in ('MATA_DOCUMENT_THEME', 'org.freedesktop.appearance', 'CosmicTheme.Mode',
                   'kdeglobals', 'gsettings', 'AppsUseLightTheme', 'AppleInterfaceStyle'):
        if symbol not in detector: errors.append(f'mata-document: missing theme detector {symbol}')
if not all(x in doc_main for x in ('theme_detection::resolve()', 'theme_detection::detect_system()',
    'theme_rx.try_recv()', 'save_button.set_child(', 'tool.button.set_child(')):
    errors.append('mata-document: automatic theme selection and live icon updates are not integrated')
if 'format!("{theme:?}:{source}")' not in doc_main:
    errors.append('mata-document: theme-aware icon cache missing')

if errors:
    for item in errors:print('ERROR:',item)
    print(f'FAILED: {len(errors)} errors across {len(packages)} crates')
    sys.exit(1)
print(f'PASS: {len(packages)} crates, package/folder/dependency/feature/import references validated, {len(removed)} removed packages still absent.')
