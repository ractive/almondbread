import AlmondbreadCore
import SwiftUI
import ImageIO
import UniformTypeIdentifiers

/// The control center: every per-window setting in grouped, searchable sections (kb:
/// decisions/controls, "Control center"). A trailing sidebar on the Mac and iPad (toolbar
/// button, ⌘I), a sheet on iPhone. App-wide preferences live in the Settings window (⌘,).
///
/// Performance (kb: platform/swiftui-performance): the sidebar holds only what can be set, so
/// nothing in it follows the 10 Hz stats (live values are in the HUD and its developer
/// details); buttons read change-only model properties. This body reads no model property,
/// each section is its own view struct (equal inputs: SwiftUI skips its body when another
/// section opens or the query changes). On the Mac the sections sit in a plain `VStack` styled
/// like a grouped form. Not a `LazyVStack`: its prefetch / placement cycle twice hung the main
/// thread for good (a menu open, then opening and scrolling the sidebar), and a reveal
/// animation laid it out at every width from 0 up. The content is laid out at the sidebar's
/// settled width (`LayoutWidth`), so the show / hide animation costs no layouts. iOS keeps its
/// `Form` (a collection view, lazy). The search field keeps its own text; the sections filter
/// on a debounced copy.
struct ControlCenterView: View {
    let model: ExplorerModel
    /// Closes the pane (iPhone and iPad: the ✕ next to the search field); nil on the Mac.
    var close: (() -> Void)? = nil
    /// The applied query (the field's text, debounced).
    @State private var query = UIDemo.search ?? ""
    @State private var confirmReset = false
    #if os(macOS)
    /// Empty space at the end left by a collapse (see `sections`) and the sections' height before it.
    @State private var slack: CGFloat = 0
    @State private var contentHeight: CGFloat = 0
    private static let scrollSpace = "controlCenterScroll"
    #endif
    /// The Places grid's selection, shared with the pinned details bar. It lives as long as
    /// the control center does: closing the control center clears it.
    @State private var places = PlaceSelection(id: UIDemo.selectedPlace)
    /// The headers and Mores flipped during the current search (the stored state stays).
    @State private var searchToggles = SearchToggles()
    /// The width the content is laid out at (the tile grids' columns follow it): the sidebar's
    /// width once it has settled for 150 ms.
    @State private var layoutWidth: CGFloat = 330
    /// The sidebar's width as it is now (it animates when the sidebar shows or hides).
    @State private var liveWidth: CGFloat = 330
    /// The inspector column's minimum width (ContentView) and the narrowest width the content
    /// is laid out at.
    static let minimumWidth: CGFloat = 290
    /// The collapsed sections. A new user starts with Controls (the key reference) and
    /// Iterations & quality collapsed; a saved state wins (kb: decisions/controls "Control center").
    @AppStorage("controlCenter.collapsed", store: TestLaunch.ephemeral ? UIDemo.defaults : .standard)
    private var collapsedStorage = ControlSection.collapsedByDefault

    var body: some View {
        let _ = UIPerf.body("ControlCenterView")
        let collapsed = Set(collapsedStorage.split(separator: ",").map(String.init))
        let searching = !query.isEmpty
        // (Sections a formula has nothing for are hidden, F2: places and tours are Mandelbrot.)
        // A place or palette whose name matches shows its section whatever its title (the
        // search covers every fractal's places; `ControlSearch`).
        let placeHit = searching && !ControlSearch.places(ExplorerModel.allPlaces, query: query).isEmpty
        let paletteHit = searching && !ControlSearch.palettes(model.library, query: query).isEmpty
        let shown = ControlSection.allCases.filter {
            ($0.matches(query) && $0.offered(by: model)) || ($0 == .places && placeHit) || ($0 == .look && paletteHit)
        }
        // A search opens every matching section; a header click during it flips just that one
        // for this search (`SearchToggles`), so the stored state comes back when it is cleared.
        let closed = Set(shown.map(\.rawValue).filter { raw in
            !SearchToggles.isOpen(stored: !collapsed.contains(raw), searchDefault: true,
                                  searching: searching, flipped: searchToggles.isFlipped(raw))
        })
        let matchKey = SearchToggles.matchKey(searching: searching, shown: shown,
                                              forcedMores: shown.filter { $0.moreMatches(query) })
        let placesOpen = shown.contains(.places) && !closed.contains(ControlSection.places.rawValue)
        // The fractal row is pinned, so a search for it ("multibrot", "power") marks it instead
        // of showing a section.
        let fractalHit = searching && ControlSearch.fractalMatches(query)
        sections(shown: shown, collapsed: closed, fractalHit: fractalHit)
        .environment(searchToggles)
        .onChange(of: matchKey, initial: true) {
            searchToggles.searching = searching
            searchToggles.reset()
        }
        .controlSize(.small)
        .safeAreaInset(edge: .top, spacing: 0) {
            VStack(spacing: 0) {
                ControlSearchField(model: model, query: $query, close: close)
                FractalRow(model: model, highlighted: fractalHit)
                Divider()
            }
        }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            // The selected place stays readable however the grid is scrolled (user,
            // 2026-10-21: the details under the 80-tile grid were off screen).
            PlaceDetailsBar(model: model, selection: places, sectionOpen: placesOpen) {
                resetAllBar
            }
        }
        .onReceive(UIPerf.actions) { action in
            if case .toggle(let s) = action { withAnimation(.snappy(duration: 0.2)) { toggle(s) } }
        }
        #if os(macOS)
        // AppKit's reveal and hide animations (and a drag of the divider) give the sidebar a
        // new width every frame, from 0 to its own. The content keeps the width it was laid
        // out at (clipped) and follows once the width has settled: one layout, not one per
        // frame (kb: platform/swiftui-performance).
        .modifier(LayoutWidth(width: layoutWidth))
        // On the Mac the canvas runs on under the sidebar: the π panel (bottom right), the
        // cosmic scale's right edge and the cockpit (centred in what stays visible) keep left
        // of it while it shows.
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: {
            model.controlCenterWidth = Double($0)
            liveWidth = $0
        }
        .task(id: liveWidth) {
            let w = max(liveWidth, ControlCenterView.minimumWidth)
            guard w != layoutWidth else { return }
            try? await Task.sleep(for: .milliseconds(150))
            if !Task.isCancelled { layoutWidth = w }
        }
        .onDisappear { model.controlCenterWidth = 0 }
        #endif
    }

    #if os(macOS)
    private func sections(shown: [ControlSection], collapsed: Set<String>, fractalHit: Bool) -> some View {
        ScrollViewReader { proxy in
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 0) {
                // Not lazy: see the type's comment (kb: platform/swiftui-performance).
                ForEach(shown) { section in
                    let open = !collapsed.contains(section.rawValue)
                    ControlSectionHeader(section: section, model: model, open: open) {
                        withAnimation(.snappy(duration: 0.2)) { toggle(section) }
                    }
                    .tourTarget(section.tourID, model: model, cornerRadius: 6)
                    .id(section.tourID)
                    .padding(.horizontal, 6)
                    .padding(.bottom, open ? 6 : 8)
                    if open {
                        ControlCardRows {
                            ControlSectionRows(section: section, model: model, places: places,
                                               query: section.searchesNames ? query : "",
                                               moreForced: section.moreMatches(query))
                        }
                        .modifier(PlacesOnScreen(selection: places, active: section == .places))
                        if let f = section.footer {
                            Text(f)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .fixedSize(horizontal: false, vertical: true)
                                .padding(.horizontal, 6)
                                .padding(.top, 6)
                        }
                        Color.clear.frame(height: 16)
                    }
                }
                if shown.isEmpty && !fractalHit {
                    Text("No setting matches “\(query)”.").foregroundStyle(.secondary)
                }
            }
            .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { contentHeightChanged($0) }
            // Slack after a collapse: the height a section or a More gave up stays as empty space
            // at the end, so the scroll view need not move (the rows below come up, nothing above
            // moves down). It goes once it is scrolled out of view, as content grows back, or
            // with a new search.
            Color.clear.frame(height: slack)
                .onGeometryChange(for: Bool.self) { g in
                    g.frame(in: .named(Self.scrollSpace)).minY > g.bounds(of: .named(Self.scrollSpace))?.height ?? .infinity
                } action: { below in
                    if below { slack = 0 }
                }
            }
            .padding(.horizontal, 14)
            .padding(.top, 10)
            .background(TourSectionScroller(model: model, proxy: proxy))
        }
        .coordinateSpace(.named(Self.scrollSpace))
        .onChange(of: query) { slack = 0 }
        .task {
            // UI demo scenes that show the end of a section (`places-bottom`).
            guard let target = UIDemo.scrollTarget else { return }
            try? await Task.sleep(for: .milliseconds(600))
            proxy.scrollTo(target, anchor: .bottom)
        }
        }
        .labeledContentStyle(ControlRowLabeledStyle())
        .toggleStyle(ControlRowToggleStyle())
        .environment(\.controlRowWidth, max(100, layoutWidth - 48))
    }
    #else
    private func sections(shown: [ControlSection], collapsed: Set<String>, fractalHit: Bool) -> some View {
        ScrollViewReader { proxy in
            Form {
                ForEach(shown) { section in
                    let open = !collapsed.contains(section.rawValue)
                    Section {
                        if open {
                            ControlSectionRows(section: section, model: model, places: places,
                                               query: section.searchesNames ? query : "",
                                               moreForced: section.moreMatches(query))
                        }
                    } header: {
                        ControlSectionHeader(section: section, model: model, open: open) {
                            withAnimation(.snappy(duration: 0.2)) { toggle(section) }
                        }
                        .tourTarget(section.tourID, model: model, cornerRadius: 8)
                    } footer: {
                        if open, let f = section.footer {
                            Text(f)
                        }
                    }
                    .id(section.tourID)
                }
                if shown.isEmpty && !fractalHit {
                    Text("No setting matches “\(query)”.").foregroundStyle(.secondary)
                }
            }
            .formStyle(.grouped)
            .background(TourSectionScroller(model: model, proxy: proxy))
        }
    }
    #endif

    #if os(macOS)
    /// The sections got shorter (a collapse): the difference becomes slack; taller: slack shrinks.
    private func contentHeightChanged(_ h: CGFloat) {
        defer { contentHeight = h }
        guard contentHeight > 0 else { return }
        slack = h < contentHeight ? slack + contentHeight - h : max(0, slack - (h - contentHeight))
    }
    #endif

    private func toggle(_ s: ControlSection) {
        if searchToggles.searching {
            searchToggles.toggle(s.rawValue)
            return
        }
        var c = Set(collapsedStorage.split(separator: ",").map(String.init))
        if c.contains(s.rawValue) { c.remove(s.rawValue) } else { c.insert(s.rawValue) }
        collapsedStorage = c.sorted().joined(separator: ",")
    }

    /// "Reset all…" at the bottom: settings only, or settings and the view.
    private var resetAllBar: some View {
        HStack {
            Spacer()
            Button(role: .destructive) {
                confirmReset = true
            } label: {
                Label("Reset all…", systemImage: "arrow.counterclockwise.circle")
            }
            .help("Every setting of this window back to the defaults")
            .confirmationDialog("Reset all settings?", isPresented: $confirmReset, titleVisibility: .visible) {
                Button("Reset settings") { model.reset(.all) }
                Button("Reset settings and view") { model.reset(.all, withView: true) }
                Button("Cancel", role: .cancel) {}
            } message: {
                Text("Look, palette, lighting, interior, overlays, iterations and quality go back to the defaults. App-wide preferences stay (Settings, ⌘,).")
            }
            Spacer()
        }
        .padding(.vertical, 8)
        .background(.bar)
    }
}

/// The search field. It owns the typed text, so a keystroke re-renders only the field; the
/// form gets the text as `query` once typing pauses (120 ms; clearing applies at once).
/// Filtering is cheap, but every changed query inserts or removes whole sections, and the
/// first character opens every matching one (measured 0.5-0.8 s per such keystroke before).
struct ControlSearchField: View {
    let model: ExplorerModel
    @Binding var query: String
    /// The ✕ at the trailing end (touch: the sheet's swipe down and the toolbar button are not
    /// obvious); pinned with the field, so it never scrolls away.
    var close: (() -> Void)? = nil
    @State private var text = UIDemo.search ?? ""
    @FocusState private var focused: Bool

    var body: some View {
        let _ = UIPerf.body("ControlSearchField")
        HStack(spacing: 10) {
            HStack(spacing: 6) {
                Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
                TextField("Search settings", text: $text)
                    .textFieldStyle(.plain)
                    .focused($focused)
                    #if os(iOS)
                    // A hardware keyboard's Esc (iPad): clear and end editing, as on the Mac.
                    .onKeyPress(.escape) {
                        model.escapeSearch()
                        return .handled
                    }
                    #endif
                if !text.isEmpty {
                    Button {
                        text = ""
                    } label: {
                        Image(systemName: "xmark.circle.fill").foregroundStyle(.secondary)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Clear search")
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 7)
            .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            if let close {
                Button(action: close) {
                    Image(systemName: "xmark.circle.fill")
                        .font(.title2)
                        .symbolRenderingMode(.hierarchical)
                        .foregroundStyle(.secondary)
                        .frame(minWidth: 44, minHeight: 44)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Close control center")
                .accessibilityIdentifier("control-center-close")
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(.bar)
        .task(id: text) {
            let q = text.trimmingCharacters(in: .whitespaces)
            if !q.isEmpty { try? await Task.sleep(for: .milliseconds(120)) }
            if !Task.isCancelled, q != query { query = q }
        }
        .onReceive(UIPerf.actions) { action in
            if case .search(let s) = action { text = s }
        }
        // ⌘F (Edit menu): also when the sidebar has only just opened for it.
        .onChange(of: model.searchFocusRequest) { takeFocus() }
        // Esc (Mac: the canvas's key monitor, which also gives the canvas the keys; iPad with a
        // keyboard: `onKeyPress`) clears the text and ends editing.
        .onChange(of: focused) { model.searchFieldFocused = focused }
        .onChange(of: model.searchEscapeRequest) {
            text = ""
            #if os(iOS)
            focused = false
            #endif
            // (Mac: the canvas is first responder already; `focused = false` here would make
            // SwiftUI resign it again, to the window.)
        }
        .onDisappear { model.searchFieldFocused = false }
        .onAppear { if model.searchFocusPending { takeFocus() } }
    }

    private func takeFocus() {
        model.searchFocusPending = false
        DispatchQueue.main.async { focused = true }
    }
}

/// A section's header: title (tap: open or close) and its ↺.
struct ControlSectionHeader: View {
    let section: ControlSection
    let model: ExplorerModel
    let open: Bool
    let toggle: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            Button(action: toggle) {
                HStack {
                    // Primary and headline weight: Form section headers default to small secondary text,
                    // which read weaker than the rows inside (user, 2026-10-16).
                    Label(section.title, systemImage: section.symbol)
                        .font(.headline)
                        .foregroundStyle(.primary)
                    Spacer()
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityAddTraits(.isHeader)
            .accessibilityHint(open ? "Collapses the section" : "Expands the section")
            if let topic = section.helpTopic {
                HelpButton(topic: topic)
            }
            if let scope = section.resetScope {
                Button {
                    model.reset(scope)
                } label: {
                    Image(systemName: "arrow.counterclockwise").font(.caption)
                }
                .buttonStyle(.plain)
                .foregroundStyle(.secondary)
                .help("Reset \(section.title.lowercased()) to the defaults")
                .accessibilityLabel("Reset \(section.title)")
            }
            // The chevron at the far right (user, 2026-10-17); it toggles like the title.
            Button(action: toggle) {
                Image(systemName: "chevron.right")
                    .rotationEffect(.degrees(open ? 90 : 0))
                    .foregroundStyle(.primary)
                    .font(.footnote.weight(.semibold))
                    .frame(width: 16)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityHidden(true)
        }
    }
}

/// One section's rows. Its inputs are a section and the model reference, so SwiftUI skips its
/// body unless the model properties it read change (an accordion toggle used to re-run every
/// open section's rows, the palette grid and the thumbnail grid included).
struct ControlSectionRows: View {
    let section: ControlSection
    let model: ExplorerModel
    /// The Places selection (one object for the control center's life, so the inputs stay equal).
    let places: PlaceSelection
    /// The search query for a section that searches names (Places, Look's palettes; empty for
    /// the others, so a new query leaves their inputs equal).
    var query = ""
    /// A search matches a row behind the section's More: it shows open.
    var moreForced = false

    var body: some View {
        let _ = UIPerf.body("rows.\(section.rawValue)")
        switch section {
        case .places: PlacesSection(model: model, selection: places, query: query)
        case .tours: ToursSectionRows(model: model)
        case .view: ViewSectionRows(model: model, moreForced: moreForced)
        case .journey: JourneySectionRows(model: model, moreForced: moreForced)
        case .look: LookSectionRows(model: model, query: query, moreForced: moreForced)
        case .lighting: LightingSectionRows(model: model, moreForced: moreForced)
        case .interior: InteriorSectionRows(model: model, moreForced: moreForced)
        case .overlays: OverlaySectionRows(model: model, moreForced: moreForced)
        case .quality: QualitySectionRows(model: model, moreForced: moreForced)
        case .controls: ControlsHelp(model: model)
        }
    }
}

// MARK: Navigation

/// Essential: Copy / Paste / Reset, Back / Forward and (Mac) Two-finger scroll; the rest behind
/// More (kb: decisions/controls "Essential rows and More"). Reset stays visible: the
/// walkthrough points touch users at it.
struct ViewSectionRows: View {
    @Bindable var model: ExplorerModel
    var moreForced = false
    @State private var showFlyTo = false

    var body: some View {
        let _ = UIPerf.body("section.view")
        FitRow {
            Button {
                Clipboard.copy(model.coordinatesText)
            } label: {
                Label("Copy", systemImage: "doc.on.doc")
            }
            .help("Copy the centre and scale at full precision (⇧⌘C)")
            #if os(iOS)
            // The system paste control: no "Allow Paste?" prompt on every tap (QA 2026-10-18).
            PasteButton(payloadType: String.self) { strings in
                Task { @MainActor in model.pasteCoordinates(text: strings.first) }
            }
            .buttonBorderShape(.capsule)
            #else
            Button {
                model.pasteCoordinates()
            } label: {
                Label("Go to copied", systemImage: "arrow.right.doc.on.clipboard")
            }
            .help("Jump to coordinates on the clipboard (⇧⌘V)")
            #endif
            Spacer()
            Button {
                model.reset()
            } label: {
                Label("Reset", systemImage: "arrow.counterclockwise")
            }
            .help("Back to the whole set (R, ⌘0)")
        }
        #if os(iOS)
        .buttonStyle(.borderless)
        #endif
        // The travel journal (kb: decisions/controls "Travel journal"): the places visited, back
        // and forward like undo (⌘Z, ⇧⌘Z / ⌘Y). Travel only: settings stay.
        FitRow {
            Button {
                model.journalBack()
            } label: {
                Label("Back", systemImage: "arrow.uturn.backward")
            }
            .disabled(!model.canGoBack)
            .help("Back to the previous place you visited (⌘Z); settings stay as they are")
            .accessibilityIdentifier("journal-back")
            Button {
                model.journalForward()
            } label: {
                Label("Forward", systemImage: "arrow.uturn.forward")
            }
            .disabled(!model.canGoForward)
            .help("Forward again to where you went back from (⇧⌘Z or ⌘Y)")
            .accessibilityIdentifier("journal-forward")
        }
        #if os(iOS)
        .buttonStyle(.borderless)
        #endif
        #if os(macOS)
        // Essential on the Mac: a trackpad user who expects zoom has no other place to find it.
        Picker("Two-finger scroll", selection: $model.scrollZooms) {
            Text("Pan").tag(false)
            Text("Zoom").tag(true)
        }
        .help("On a trackpad: two fingers pan the view, or zoom (up and down; then click and drag pans). Pinching always zooms")
        #endif
        MoreRows(section: .view, model: model, forced: moreForced) {
            ControlRows.picker("Edge panning", selection: $model.edgePanning) {
                ForEach(EdgePanning.allCases) { Text($0.label).tag($0) }
            }
            // Autopilot and fly-to (kb: decisions/controls, autopilot).
            FitRow {
                Button {
                    model.toggleAutopilot()
                } label: {
                    Label(model.autopilotActive ? "Stop" : "Autopilot",
                          systemImage: model.autopilotActive ? "airplane.circle.fill" : "airplane")
                }
                .help("Zoom on continuously towards interesting places; click pauses, Esc stops, steering hands over to cruise (Z)")
                if model.flightActive {
                    Button {
                        model.stopMotion()
                    } label: {
                        Label("Stop flight", systemImage: "stop.circle")
                    }
                    .help("Stop the flight (Esc)")
                } else {
                    Button {
                        showFlyTo = true
                    } label: {
                        Label("Fly to…", systemImage: "location.north.line")
                    }
                    .help("Fly to coordinates (⌘G)")
                    Button {
                        model.flyToCopiedCoordinates()
                    } label: {
                        Label("Copied", systemImage: "doc.on.clipboard")
                    }
                    .help("Fly to coordinates on the clipboard (⇧⌘G)")
                }
            }
            #if os(iOS)
            .buttonStyle(.borderless)
            #endif
            .sheet(isPresented: $showFlyTo) { FlyToSheet(model: model, isPresented: $showFlyTo) }
            // (The target lock's finder is the Mandelbrot journey's: hidden without that content, F3.)
            if model.has(\.content) {
                Toggle("Autopilot target lock", isOn: $model.autopilotLock)
                    .help("The autopilot aims at spirals, branch points and minibrots, and backs out of featureless regions. Off: it flies straight. Steering yourself hands over to cruise")
            }
            // Lane assist (kb: decisions/controls "Driving levels"): cruise only, key L.
            Toggle("Lane assist", isOn: $model.laneAssist)
                .help("While you cruise (E / Q), keeps you out of featureless areas with a gentle nudge; your steering always wins (L)")
            #if os(iOS)
            // Seek minibrot about the view (N; kb: decisions/controls "Seek minibrot"): the long-press
            // menu's "Seek Minibrot Here" needs a still picture, this works while flying too (it was
            // the round autopilot button's menu until 2026-10-18).
            if model.has(\.seek) {
                Button {
                    model.seekMinibrot()
                } label: {
                    Label("Seek minibrot", systemImage: "scope")
                }
                .buttonStyle(.borderless)
            }
            #endif
            // View rotation (kb: decisions/controls, "View rotation"); the compass shows the angle.
            LabeledContent("Rotation") {
                Button("Straighten") { model.straighten() }
                    .disabled(!model.viewTurned)
                    .help("Turn the view upright (/); , and . turn it, so does a trackpad rotation")
            }
        }
    }
}

/// The fractal row, pinned under the search field (kb: decisions/controls "Control center",
/// Fractal row): the fractal decides which places, tours and settings exist, so it is no row of
/// a section. Its family (Mandelbrot, Multibrot (F3, kb: plans/f3-multibrot), Mandelbar (F5),
/// Burning Ship (F6); a later family is one more entry of Rust's `formula_options()`) and, for a
/// family of several formulas, which one (Multibrot's power 3 ... 8). A switch goes to the new
/// set's whole view. In a Julia view (F4) it names the set the Julia set belongs to and offers
/// the way back instead of the pickers (as before: no fractal is picked in a Julia view; Julia
/// sets are opened and left through the twin / Julia panel).
struct FractalRow: View {
    let model: ExplorerModel
    /// The search matches the row (`ControlSearch.fractalMatches`).
    var highlighted = false

    var body: some View {
        let _ = UIPerf.body("FractalRow")
        Group {
            if model.isJulia { julia } else { pickers }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 6)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(highlighted ? Color.accentColor.opacity(0.18) : Color.clear)
        .background(.bar)
    }

    /// The current family's name ("Mandelbrot"; a Julia view's family is the Mandelbrot set's).
    private var familyTitle: String {
        ExplorerModel.formulaFamilies.first { $0.id == model.formulaFamily }?.title ?? "Mandelbrot"
    }

    private var title: some View {
        // Never hyphenated ("Frac-tal" in a narrow pane): the row drops or wraps around it.
        Text("Fractal").font(.headline).fixedSize()
    }

    private var pickers: some View {
        let family = Binding(get: { model.formulaFamily }, set: { model.selectFormulaFamily($0) })
        let formula = Binding(get: { model.formula }, set: { model.selectFormula($0) })
        let options = ExplorerModel.formulaOptions(family: model.formulaFamily)
        // One line; where "Fractal" and both menus do not fit (an iPad mini's portrait pane with
        // Multibrot's power menu), the menus alone: the family's name says what it is.
        return ViewThatFits(in: .horizontal) {
            HStack(spacing: 8) {
                title
                Spacer(minLength: 4)
                menus(family: family, formula: formula, options: options)
            }
            HStack(spacing: 8) {
                Spacer(minLength: 0)
                menus(family: family, formula: formula, options: options)
            }
        }
    }

    @ViewBuilder
    private func menus(family: Binding<String>, formula: Binding<String>, options: [FormulaOption]) -> some View {
        Picker("Fractal", selection: family) {
            ForEach(ExplorerModel.formulaFamilies, id: \.id) { Text($0.title).tag($0.id) }
        }
        .labelsHidden()
        .fixedSize()
        .help("The set to explore: the Mandelbrot set (z² + c), a Multibrot set (zᵖ + c), the Mandelbar set (the Tricorn, z̄² + c) or the Burning Ship ((|x| − i|y|)² + c, shown upright)")
        .accessibilityIdentifier("formula-family")
        if options.count > 1 {
            Picker("Power", selection: formula) {
                ForEach(options, id: \.formula) { o in Text(o.title).tag(o.formula) }
            }
            .labelsHidden()
            .fixedSize()
            .help("The power p of zᵖ + c: a (p − 1)-fold symmetric set")
            .accessibilityIdentifier("formula-power")
        }
    }

    private var julia: some View {
        let name = Text("Julia set of \(familyTitle)").foregroundStyle(.secondary).lineLimit(1)
        let back = Button {
            model.backFromJulia()
        } label: {
            Label("Back to \(familyTitle)", systemImage: "arrow.uturn.backward")
        }
        .fixedSize()
        .help("Where this Julia set was opened from; another fractal is picked from there (⇧U)")
        .accessibilityIdentifier("fractal-julia-back")
        // One line where it fits, else the button under the name.
        return ViewThatFits(in: .horizontal) {
            HStack(spacing: 8) {
                title
                name.fixedSize()
                Spacer(minLength: 4)
                back
            }
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 8) { title; name }
                back
            }
        }
    }
}

// MARK: Journey & cinematic effects

/// Essential: the seed (above the button that reads it), Journey / Daily, the mode and the
/// Cinematic effects switch; the effects' four strengths behind More.
struct JourneySectionRows: View {
    @Bindable var model: ExplorerModel
    var moreForced = false
    @State private var journeySeed = ""

    var body: some View {
        let _ = UIPerf.body("section.journey")
        // (Journeys are Mandelbrot content: hidden for a formula without it, F2.)
        if model.has(\.content) {
            TextField("Seed (empty: random)", text: $journeySeed)
                .help("A journey number, e.g. 4711; started from the whole set it is the same journey everywhere")
            JourneyControls(model: model, journeySeed: journeySeed)
            Picker("Journey", selection: $model.journeyStops) {
                Text("Continuous").tag(false)
                Text("Stop at each find").tag(true)
            }
            .help("Continuous: rest a moment at each landmark and fly on. Stop at each find: stay until you choose: J next, Z deeper here, X somewhere else. J starts this mode, ⇧J the other.")
        }
        Toggle("Cinematic effects", isOn: $model.cinematic)
            .help("Spin with the zoom, palette drift with depth and slow cycling while flying (autopilot, journeys, fly-to)")
        MoreRows(section: .journey, model: model, forced: moreForced) {
            Group {
                LabeledContent("Spin") {
                    Slider(value: $model.cinematicSpin, in: -15...15) { Text("Spin") }.labelsHidden()
                        .help(String(format: "%.1f° per e-fold of zoom (×2.7)", model.cinematicSpin))
                }
                LabeledContent("Hue drift") {
                    Slider(value: $model.cinematicHueDrift, in: 0...15) { Text("Hue drift") }.labelsHidden()
                        .help(String(format: "%.1f° of hue per e-fold of depth", model.cinematicHueDrift))
                }
                LabeledContent("Phase drift") {
                    Slider(value: $model.cinematicPhaseDrift, in: 0...0.1) { Text("Phase drift") }.labelsHidden()
                        .help(String(format: "%.3f palette cycles per e-fold of depth", model.cinematicPhaseDrift))
                }
                LabeledContent("Cycling in flight") {
                    Slider(value: $model.cinematicFlyCycle, in: 0...0.1) { Text("Cycling in flight") }.labelsHidden()
                        .help(String(format: "%.3f palette cycles per second while flying", model.cinematicFlyCycle))
                }
            }
            .disabled(!model.cinematic)
        }
    }
}

/// Journey buttons (change-only model state: active, paused, seed; the HUD shows the leg,
/// phase and speed).
struct JourneyControls: View {
    let model: ExplorerModel
    let journeySeed: String

    var body: some View {
        let _ = UIPerf.body("leaf.journey")
        let active = model.journeyActive
        let suspended = model.journeySuspended
        FitRow {
            Button {
                if active && suspended {
                    model.resumeJourney()
                } else if active {
                    model.stopJourney()
                } else if let seed = ExplorerModel.journeySeed(from: journeySeed) {
                    model.startJourney(seed: seed)
                } else {
                    model.toggleJourney()
                }
            } label: {
                Label(active ? (suspended ? "Resume" : "Stop") : "Journey",
                      systemImage: active ? "sparkles.rectangle.stack.fill" : "sparkles")
            }
            .help("An endless planned flight from landmark to landmark (J); a seed above replays that journey")
            Button {
                model.startDailyJourney()
            } label: {
                Label("Daily", systemImage: "calendar")
            }
            .help("Today's journey: the same for everyone today")
            if active {
                Button {
                    model.copyJourney()
                } label: {
                    Label("Copy #\(model.journeySeed)", systemImage: "doc.on.doc")
                }
                .help("Copy the journey's seed to share it")
            }
        }
        #if os(iOS)
        .buttonStyle(.borderless)
        #endif
    }
}

// MARK: Look & colour

/// Essential: the Look, the palette (name, category, grid) and the density; the mapping's
/// fine points, colour cycling, depth tint and the import behind More.
struct LookSectionRows: View {
    @Bindable var model: ExplorerModel
    /// The search query (the palette grid then shows the palettes whose name matches).
    var query = ""
    var moreForced = false

    var body: some View {
        let _ = UIPerf.body("section.look")
        // (A menu on iOS too: the navigation-link picker had no navigation stack in the pane and
        // was disabled on the iPad, found by the BB UI test.)
        ControlRows.picker("Look", selection: Binding(get: { model.lookID }, set: { model.applyLook($0) })) {
            looks
        }
        .accessibilityIdentifier("look-picker")
        PalettePicker(model: model, query: query)
        // A user edit pivots on the view's median escape count (kb: techniques/coloring-methods,
        // "Density pivot"): the offset slider moves along.
        ControlRows.slider("Density",
                           value: ControlRows.log(Binding(get: { model.colorDensity },
                                                          set: { model.editColorDensity($0) }),
                                                  range: 0.001...1),
                           in: 0...1, text: String(format: "%.4g", model.colorDensity),
                           onEditingChanged: { model.densityEditing($0) })
        MoreRows(section: .look, model: model, forced: moreForced) {
            ControlRows.slider("Offset", value: $model.colorOffset, in: 0...1, text: String(format: "%.2f", model.colorOffset))
            Toggle("Logarithmic", isOn: $model.logColoring)
            Toggle("Smooth", isOn: $model.smoothColoring)
            Toggle("Palette anti-aliasing", isOn: $model.paletteFilter)
            Toggle("Calm dense regions", isOn: $model.denseCalm)
                .help("Where the palette cycles faster than a pixel, show a dark calm colour with thin filaments instead of grey noise")
            Toggle("Colour cycling", isOn: Binding(get: { model.cycleSpeed != 0 },
                                                   set: { model.cycleSpeed = $0 ? 0.05 : 0 }))
            if model.cycleSpeed != 0 {
                ControlRows.slider("Cycle speed", value: ControlRows.log($model.cycleSpeed, range: 0.005...0.5), in: 0...1,
                                   text: String(format: "%.3g /s", model.cycleSpeed))
            }
            ControlRows.slider("Depth tint", value: $model.depthTint, in: 0...0.2,
                               text: model.depthTint == 0 ? "off" : String(format: "%.3f / decade", model.depthTint))
            PaletteImportRow(model: model)
        }
    }
}

extension LookSectionRows {
    private var looks: some View {
        ForEach(model.availableLooks, id: \.id) { l in
            HStack {
                if let p = ExplorerModel.palettes.first(where: { $0.id == l.paletteId }) {
                    PaletteSwatch(palette: p).frame(width: 64, height: 12)
                }
                Text(l.name)
            }
            .tag(l.id)
        }
    }
}

// MARK: Lighting

/// Essential: the three strengths (Shading, Outline, Glow); their sub-knobs behind More.
struct LightingSectionRows: View {
    @Bindable var model: ExplorerModel
    var moreForced = false

    var body: some View {
        let _ = UIPerf.body("section.lighting")
        ControlRows.slider("Shading", value: $model.shading, in: 0...1, text: String(format: "%.2f", model.shading))
        ControlRows.slider("Outline", value: $model.outline, in: 0...1, text: String(format: "%.2f", model.outline))
        ControlRows.slider("Glow", value: $model.glow, in: 0...0.5, text: String(format: "%.2f", model.glow))
        MoreRows(section: .lighting, model: model, forced: moreForced) {
            ControlRows.slider("Light angle", value: $model.lightAngle, in: 0...360, text: String(format: "%.0f°", model.lightAngle))
                .disabled(model.shading == 0)
            ControlRows.slider("Light height", value: $model.lightHeight, in: 0.2...4, text: String(format: "%.2f", model.lightHeight))
                .disabled(model.shading == 0)
            ControlRows.slider("Highlight", value: $model.specular, in: 0...1, text: String(format: "%.2f", model.specular))
                .disabled(model.shading == 0)
            ControlRows.slider("Outline width", value: $model.outlineWidth, in: 0.5...6, text: String(format: "%.1f px", model.outlineWidth))
                .disabled(model.outline == 0)
            ControlRows.color("Outline colour", ControlRows.linearColor(model, \.edgeColor))
                .disabled(model.outline == 0)
            ControlRows.slider("Glow width", value: $model.glowWidth, in: 1...40, text: String(format: "%.0f px", model.glowWidth))
                .disabled(model.glow == 0)
            ControlRows.color("Glow colour", ControlRows.linearColor(model, \.glowColor))
                .disabled(model.glow == 0)
        }
    }
}

// MARK: Interior

/// Essential: the colouring with its one knob (Colour for flat, Pattern density otherwise);
/// Stripes (angle) and interior detection behind More.
struct InteriorSectionRows: View {
    @Bindable var model: ExplorerModel
    var moreForced = false

    var body: some View {
        let _ = UIPerf.body("section.interior")
        ControlRows.picker("Colouring", selection: $model.interiorColoring) {
            ForEach(InteriorColoring.allCases) { Text($0.label).tag($0) }
        }
        Text(model.interiorColoring.help)
            .font(.caption)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
        if model.interiorColoring == .flat {
            ControlRows.color("Colour", ControlRows.linearColor(model, \.interiorColor))
        } else {
            // (Not "Density": that is the palette's, in Look & colour.) How tightly the interior
            // colouring's rings and bands repeat.
            ControlRows.slider("Pattern density", value: ControlRows.log($model.interiorDensity, range: 0.02...4), in: 0...1,
                               text: String(format: "%.3g", model.interiorDensity))
                .help("How closely the interior colouring's rings and bands repeat")
        }
        MoreRows(section: .interior, model: model, forced: moreForced) {
            if model.interiorColoring == .angle {
                ControlRows.slider("Stripes", value: $model.interiorStripes, in: 0...32,
                                   text: String(format: "%.0f", model.interiorStripes))
            }
            Toggle("Interior detection", isOn: $model.interiorDetection)
        }
    }
}

// MARK: Overlays

/// Essential: Orbit, Mini-map, the Julia panel and the Julia twin; their tuning and the display
/// preferences behind More. (In a Julia view the way back is the pinned fractal row's Back.)
struct OverlaySectionRows: View {
    @Bindable var model: ExplorerModel
    var moreForced = false

    var body: some View {
        let _ = UIPerf.body("section.overlays")
        if model.has(\.orbitOverlay) {
            Toggle("Orbit of the point under the pointer", isOn: $model.orbitEnabled)
        }
        Toggle("Mini-map", isOn: $model.mapEnabled)
        if model.has(\.juliaPanel) {
            Toggle("Julia set of that point", isOn: $model.juliaPanelToggle)
                .help("A panel next to the mini-map with the Julia set of the orbit's point: start the rule from every z with that c fixed. Connected inside the set, dust outside. Click it to open that Julia set, or ⌘-click a point of the picture to open its Julia set (U)")
            // F4 (kb: plans/f4-deep-julia): the deep Julia set of the view's centre.
            Button {
                model.juliaTwin()
            } label: {
                Label("Julia twin of this view", systemImage: InlineIcons.symbol("julia"))
            }
            .help("The Julia set of the view's centre, framed like the view: near a Misiurewicz point the two look alike. Zoom it as deep as you like (⇧U)")
            .accessibilityIdentifier("julia-twin")
        } else if model.isJulia {
            // In a Julia view the same switch shows the Mandelbrot panel (the way back).
            Toggle("Mandelbrot panel", isOn: $model.juliaPanelToggle)
                .help("In the Julia panel's place: the Mandelbrot view this Julia set was opened from, marked at k. Click it to return (U)")
        }
        MoreRows(section: .overlays, model: model, forced: moreForced) {
            if model.orbitEnabled && model.has(\.orbitOverlay) {
                ControlRows.slider("Iterates", value: ControlRows.log(ControlRows.int($model.orbitLength), range: 1...1000), in: 0...1,
                                   text: "\(model.orbitLength)")
            }
            if model.mapEnabled {
                LabeledContent("Mini-map levels") {
                    HStack(spacing: 4) {
                        Text("\(model.mapLevels)").monospacedDigit()
                        Stepper("Mini-map levels", value: $model.mapLevels, in: 1...4).labelsHidden()
                    }
                }
                .help("The whole set plus up to three deeper maps at thousands of times the zoom")
                ControlRows.slider("Mini-map size", value: $model.mapSize, in: 100...320, text: String(format: "%.0f pt", model.mapSize))
                ControlRows.picker("Mini-map corner", selection: $model.mapPlacement) {
                    ForEach(MapPlacement.allCases) { Text($0.label).tag($0) }
                }
            }
            CosmicScaleRows(model: model)
            Toggle("Always show compass", isOn: $model.compassAlways)
                .help("Otherwise the compass shows while the view is turned. Click it to turn north, drag it to turn")
            Toggle("Autopilot target marker", isOn: $model.targetMarker)
                .help("A small ring where the autopilot or a journey is heading, with the pointer's wished direction as a faint line")
            Toggle("Status (HUD)", isOn: $model.showHUD)
            Toggle("Developer details in the HUD", isOn: $model.hudDetails)
                .disabled(!model.showHUD)
                .help("Iterations, GPU time, render size, reference orbit, kernel and render state")
        }
    }
}

// MARK: Iterations & quality

struct QualitySectionRows: View {
    @Bindable var model: ExplorerModel
    var moreForced = false

    /// The Upper limit choices (Rust's `UPPER_LIMIT_CHOICES`, which the HUD's "Raise to" offer
    /// steps through).
    private static let iterationCaps = upperLimitChoices().map { Int($0) }

    var body: some View {
        let _ = UIPerf.body("section.quality")
        Toggle("Automatic iterations", isOn: $model.autoIterations)
        if !model.autoIterations {
            // One row for the one value: the number (typed) over its log slider.
            VStack(alignment: .leading, spacing: 2) {
                HStack {
                    Text("Maximum")
                    Spacer()
                    TextField("Maximum", value: $model.manualIterations, format: .number)
                        .multilineTextAlignment(.trailing)
                        .monospacedDigit()
                        .frame(maxWidth: 110)
                        .accessibilityIdentifier("manual-iterations")
                }
                Slider(value: ControlRows.log(ControlRows.int($model.manualIterations), range: 100...10_000_000,
                                              snap: ControlRows.twoDigits), in: 0...1) {
                    Text("Maximum")
                }
                .labelsHidden()
                .accessibilityLabel("Maximum iterations")
                .accessibilityValue(model.manualIterations.formatted())
                if model.manualIterations > Int(maxIterationLimit()) {
                    // The renderer clamps (crates/almondbread-core MAX_ITERATION_LIMIT).
                    Text("Renders with \(Int(maxIterationLimit()).formatted()): the most the GPU counts exactly.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        // The tour on iterations and the fog (topic `iterations`), once one is bundled.
        if ExplorerModel.tour(forTopic: "iterations") != nil {
            TourTopicLink(model: model, topic: "iterations", label: nil)
        }
        MoreRows(section: .quality, model: model, forced: moreForced) {
            if model.autoIterations {
                // Bounds only the automatic limit (a manual limit is used as set).
                ControlRows.picker("Upper limit", selection: $model.maxIterationsCap) {
                    ForEach(Self.iterationCaps, id: \.self) { Text($0.formatted()).tag($0) }
                }
                .help("The most iterations automatic mode may use. It raises the limit only as far as the view needs.")
            }
            ControlRows.picker("Anti-aliasing", selection: $model.supersampling) {
                ForEach(Supersampling.allCases) { Text($0.label).tag($0) }
            }
            Toggle("Interactive preview", isOn: $model.interactivePreview)
        }
    }
}

// MARK: Grouped-form look on the Mac (kb: platform/swiftui-performance)

#if os(macOS)
/// A section's rows on a rounded background with separators between them, like a grouped
/// `Form` section. Each row is its own child of the sidebar's stack (with its slice of the card:
/// rounded corners on the first and the last). Rows marked `controlRowTight` (tile grid rows) sit closer and without a separator.
struct ControlCardRows<Content: View>: View {
    @ViewBuilder var content: Content

    var body: some View {
        if #available(macOS 15.0, *) {
            Group(subviews: content) { rows in
                let last = rows.count - 1
                ForEach(Array(rows.enumerated()), id: \.element.id) { i, row in
                    let tight = row.containerValues.controlRowTight
                    let afterTight = i > 0 && rows[i - 1].containerValues.controlRowTight
                    let r: CGFloat = 8
                    row
                        .padding(.horizontal, 10)
                        .padding(.top, tight && afterTight ? 0 : 7)
                        .padding(.bottom, tight ? 6 : 7)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .overlay(alignment: .top) {
                            if i > 0 && !(tight && afterTight) { Divider().padding(.leading, 10) }
                        }
                        .background(.background.secondary, in: UnevenRoundedRectangle(
                            topLeadingRadius: i == 0 ? r : 0, bottomLeadingRadius: i == last ? r : 0,
                            bottomTrailingRadius: i == last ? r : 0, topTrailingRadius: i == 0 ? r : 0,
                            style: .continuous))
                }
            }
        } else {
            VStack(alignment: .leading, spacing: 10) { content }
                .padding(.horizontal, 10).padding(.vertical, 8)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(.background.secondary, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        }
    }
}

@available(macOS 15.0, *)
extension ContainerValues {
    /// A row of a tile grid: no separator to the row above it if that is one too.
    @Entry var controlRowTight = false
}

/// Label leading, value or control trailing (a grouped form row).
struct ControlRowLabeledStyle: LabeledContentStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(alignment: .firstTextBaseline) {
            configuration.label
            Spacer(minLength: 8)
            configuration.content
        }
    }
}

/// Label leading, a small switch trailing (a grouped form's toggle).
struct ControlRowToggleStyle: ToggleStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack {
            configuration.label
            Spacer(minLength: 8)
            Toggle(isOn: configuration.$isOn) { configuration.label }
                .toggleStyle(.switch)
                .labelsHidden()
                .controlSize(.mini)
        }
    }
}
#endif

#if os(macOS)
/// Lays its content out `width` wide whatever width it is given (clipped, leading), and reports
/// the width it is given: the control center while its width animates.
struct LayoutWidth: ViewModifier {
    let width: CGFloat

    func body(content: Content) -> some View {
        FixedWidthLayout(width: width) { content }.clipped()
    }

    private struct FixedWidthLayout: Layout {
        let width: CGFloat

        func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
            let size = subviews.first?.sizeThatFits(ProposedViewSize(width: width, height: proposal.height)) ?? .zero
            return CGSize(width: proposal.width ?? size.width, height: proposal.height ?? size.height)
        }

        func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
            subviews.first?.place(at: bounds.origin, anchor: .topLeading,
                                  proposal: ProposedViewSize(width: width, height: bounds.height))
        }
    }
}
#endif

/// Same-shape tiles in rows: on the Mac each row of the grid is a row of the sidebar's stack;
/// the columns follow the sidebar's settled width. iOS: one grid in one form row.
struct TileRows<Tile: View>: View {
    let ids: [String]
    var minWidth: CGFloat
    var spacing: CGFloat
    var aspect: CGFloat = 1
    var height: CGFloat?
    @ViewBuilder var tile: (Int) -> Tile
    @Environment(\.controlRowWidth) private var width

    var body: some View {
        #if os(macOS)
        let columns = TileGrid.columns(width: width, minWidth: minWidth, spacing: spacing)
        ForEach(Array(stride(from: 0, to: ids.count, by: columns)), id: \.self) { start in
            let row = start..<min(ids.count, start + columns)
            TileGrid(minWidth: minWidth, spacing: spacing, aspect: aspect, height: height, columns: columns) {
                // (A stale index: SwiftUI may update a row's child once more after the list shrank,
                // as when the formula changes and the places go from 80 to 3, F3 + F4.)
                ForEach(Array(row), id: \.self) { i in if i < ids.count { tile(i).id(ids[i]) } }
            }
            .modifier(TightRow())
        }
        #else
        TileGrid(minWidth: minWidth, spacing: spacing, aspect: aspect, height: height) {
            ForEach(ids.indices, id: \.self) { i in if i < ids.count { tile(i).id(ids[i]) } }
        }
        #endif
    }
}

#if os(macOS)
/// Marks a tile grid row (`ControlCardRows`: no separator between such rows).
struct TightRow: ViewModifier {
    func body(content: Content) -> some View {
        if #available(macOS 15.0, *) {
            content.containerValue(\.controlRowTight, true)
        } else {
            content
        }
    }
}
#endif

extension EnvironmentValues {
    /// The width of a control center row's content (the sidebar's width less its margins).
    @Entry var controlRowWidth: CGFloat = 282
}

// MARK: Row helpers

/// A row of labelled buttons: titles and icons where they fit, else the icons alone. The iPad's
/// sidebar next to a portrait canvas wrapped "Co py", "Re-set", "Au-topi-lot" (seen 2026-10-18).
/// The titles stay the accessibility labels.
struct FitRow<Content: View>: View {
    @ViewBuilder let content: () -> Content

    var body: some View {
        ViewThatFits(in: .horizontal) {
            // Fixed: titles that would truncate do not count as fitting.
            HStack { content() }.lineLimit(1).fixedSize(horizontal: true, vertical: false)
            HStack { content() }.labelStyle(.iconOnly)
        }
    }
}

enum ControlRows {
    /// A menu picker as a labelled row: the label leading, the menu trailing at its own size
    /// (outside a `Form` a picker puts its menu right after the label and stretches it).
    static func picker<V: Hashable, C: View>(_ title: String, selection: Binding<V>,
                                             @ViewBuilder content: () -> C) -> some View {
        LabeledContent(title) {
            Picker(title, selection: selection, content: content)
                .labelsHidden()
                .fixedSize()
        }
    }

    static func color(_ title: String, _ selection: Binding<Color>) -> some View {
        LabeledContent(title) {
            ColorPicker(title, selection: selection, supportsOpacity: false).labelsHidden()
        }
    }

    /// The model keeps colours in linear RGB (what the kernel blends in).
    @MainActor
    static func linearColor(_ model: ExplorerModel, _ path: ReferenceWritableKeyPath<ExplorerModel, SIMD3<Float>>) -> Binding<Color> {
        Binding {
            let c = model[keyPath: path]
            return Color(.sRGBLinear, red: Double(c.x), green: Double(c.y), blue: Double(c.z))
        } set: { color in
            let r = color.resolve(in: EnvironmentValues())
            model[keyPath: path] = SIMD3(r.linearRed, r.linearGreen, r.linearBlue)
        }
    }

    static func slider(_ title: String, value: Binding<Double>, in range: ClosedRange<Double>, text: String,
                       onEditingChanged: @escaping (Bool) -> Void = { _ in }) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack {
                Text(title)
                Spacer()
                Text(text).monospacedDigit().foregroundStyle(.secondary)
            }
            Slider(value: value, in: range, onEditingChanged: onEditingChanged)
                .labelsHidden()
                .accessibilityLabel(title)
                .accessibilityValue(text)
        }
    }

    static func int(_ b: Binding<Int>) -> Binding<Double> {
        Binding { Double(b.wrappedValue) } set: { b.wrappedValue = Int($0.rounded()) }
    }

    /// Maps 0...1 onto `range` logarithmically; `snap` rounds what the slider sets.
    static func log(_ b: Binding<Double>, range: ClosedRange<Double>,
                    snap: @escaping (Double) -> Double = { $0 }) -> Binding<Double> {
        let lo = Foundation.log(range.lowerBound), hi = Foundation.log(range.upperBound)
        return Binding {
            (Foundation.log(min(max(b.wrappedValue, range.lowerBound), range.upperBound)) - lo) / (hi - lo)
        } set: {
            b.wrappedValue = min(max(snap(exp(lo + $0 * (hi - lo))), range.lowerBound), range.upperBound)
        }
    }

    /// Rounds to two significant digits (1,000,000, 1,100,000, ..., 9,900,000; 100, 110, ...):
    /// readable values from a log slider, 1-10 % apart.
    static func twoDigits(_ v: Double) -> Double {
        guard v.isFinite, v > 0 else { return v }
        let step = pow(10, floor(log10(v)) - 1)
        return (v / step).rounded() * step
    }
}

/// Scrolls the control center to the section a tour step rings (`highlight: section-…`). A leaf
/// reading only the change-only `model.tour`, so the control center's body reads no model value.
struct TourSectionScroller: View {
    let model: ExplorerModel
    let proxy: ScrollViewProxy

    var body: some View {
        let target = model.tour?.highlight ?? ""
        Color.clear
            .task(id: target) {
                guard target.hasPrefix("section-") else { return }
                // After the control center opened (the same step opens it).
                try? await Task.sleep(for: .milliseconds(450))
                guard !Task.isCancelled else { return }
                withAnimation(.easeInOut(duration: 0.35)) { proxy.scrollTo(target, anchor: UnitPoint(x: 0.5, y: 0.08)) }
            }
    }
}

/// The control center's sections, in order, with what the search matches.
enum ControlSection: String, CaseIterable, Identifiable {
    case places, tours, view, journey, look, lighting, interior, overlays, quality, controls

    /// Whether the current formula has anything for this section (F2: places and tours are
    /// Mandelbrot content).
    @MainActor func offered(by model: ExplorerModel) -> Bool {
        switch self {
        // (F3: a Multibrot power has a few places of its own, but no tours.)
        case .places: model.has(\.content) || !model.currentPlaces.isEmpty
        case .tours: model.has(\.content)
        default: true
        }
    }
    var id: String { rawValue }
    /// The name a tour step rings it by (`highlight: section-places`; core `HIGHLIGHT_TARGETS`).
    var tourID: String { "section-" + rawValue }

    var title: String {
        switch self {
        case .places: "Places"
        case .tours: "Tours"
        case .view: "Navigation"
        case .journey: "Journey & cinematic effects"
        case .look: "Look & colour"
        case .lighting: "Lighting & outline"
        case .interior: "Interior"
        case .overlays: "Overlays"
        case .quality: "Iterations & quality"
        case .controls: "Controls"
        }
    }

    var symbol: String {
        switch self {
        case .places: "mappin.and.ellipse"
        case .tours: "signpost.right"
        case .view: "scope"
        case .journey: "film"
        case .look: "paintpalette"
        case .lighting: "light.max"
        case .interior: "circle.inset.filled"
        case .overlays: "square.on.square.dashed"
        case .quality: "sparkles"
        case .controls: "keyboard"
        }
    }

    /// What the header's (?) explains (kb: decisions/controls "Help"); nil: no (?).
    var helpTopic: HelpTopic? {
        switch self {
        case .quality: .iterations
        case .look: .look
        case .interior: .interior
        case .overlays: .overlays
        case .controls: .controls
        default: nil
        }
    }

    /// What the section's ↺ resets (None: nothing to reset).
    var resetScope: ExplorerModel.ResetScope? {
        switch self {
        case .view: .view
        case .journey: .cinematic
        case .look: .look
        case .lighting: .lighting
        case .interior: .interior
        case .overlays: .overlays
        case .quality: .quality
        case .places, .tours, .controls: nil
        }
    }

    /// Whether the section's rows search names too (Places: places; Look: palettes).
    var searchesNames: Bool { self == .places || self == .look }

    /// The sections a new user starts with collapsed (kb: decisions/controls "Essential rows and
    /// More"): the key reference and Iterations & quality (its one essential row, automatic
    /// iterations, is already on). Only the default: a saved open / closed state wins.
    static let collapsedByDefault = "controls,quality"

    /// What the rows behind the section's More are found by (empty: the section has no More).
    /// A search matching one of these shows the More open.
    var moreKeywords: [String] {
        switch self {
        case .places, .tours, .controls: []
        case .view: ["edge", "pan", "autopilot", "fly", "copied", "lock", "assist", "lane", "cruise", "rotation", "straighten"] + Self.touchViewKeywords
        case .journey: ["cinematic", "effects", "rotation", "rotate", "spin", "turn", "hue", "drift", "phase", "cycling"]
        case .look: ["offset", "smooth", "log", "logarithmic", "cycling", "cycle", "tint", "dense", "calm", "grey", "filter", "anti-aliasing", "import"]
        case .lighting: ["light", "angle", "height", "highlight", "specular", "width", "colour", "color", "edge"]
        case .interior: ["stripes", "angle", "detection", "atom"]
        case .overlays: ["iterates", "levels", "size", "corner", "cosmic", "scale", "universe", "planck", "compass", "target", "marker", "hud", "status", "developer", "details", "diagnostics", "gpu", "performance", "reference", "kernel", "stats", "cockpit", "gear", "speed"]
        case .quality: ["upper limit", "cap", "anti-aliasing", "aa", "samples", "preview"]
        }
    }

    /// Whether a search query matches a row behind the section's More (so it shows open).
    func moreMatches(_ query: String) -> Bool {
        let q = query.trimmingCharacters(in: .whitespaces).lowercased()
        guard !q.isEmpty else { return false }
        return moreKeywords.contains { $0.contains(q) }
    }

    /// Whether the section matches a search query (title or keywords, case-insensitive).
    func matches(_ query: String) -> Bool {
        let q = query.trimmingCharacters(in: .whitespaces).lowercased()
        guard !q.isEmpty else { return true }
        return title.lowercased().contains(q) || keywords.contains { $0.contains(q) }
    }

    /// iOS has the Seek minibrot button in View.
    #if os(iOS)
    static let touchViewKeywords = ["seek", "minibrot"]
    #else
    static let touchViewKeywords: [String] = []
    #endif

    var keywords: [String] {
        switch self {
        case .places: ["location", "bookmark", "famous", "random", "fly", "go", "catalog"]
        case .tours: ["tour", "guided", "learn", "maths", "math", "feigenbaum", "period", "doubling", "chaos", "orbit", "explain", "minibrot", "pi", "π", "neck", "cusp", "fog", "limit", "bulb", "valley", "julia", "misiurewicz", "colour", "color"]
        case .view: ["coordinates", "copy", "paste", "reset", "edge", "pan", "autopilot", "fly", "lock", "assist", "lane", "cruise", "rotation", "straighten", "scroll", "zoom", "trackpad", "two-finger", "gesture", "pinch"] + Self.touchViewKeywords
        case .journey: ["journey", "seed", "daily", "share", "stop", "stops", "find", "continuous", "cinematic", "effects", "rotation", "rotate", "spin", "turn", "hue", "drift", "cycling", "landmark", "minibrot", "spiral", "julia"]
        case .look: ["palette", "look", "colour", "color", "density", "offset", "smooth", "log", "cycling", "tint", "dense", "grey", "filter", "import", "calm", "anti-aliasing"]
        case .lighting: ["shading", "light", "angle", "height", "highlight", "specular", "outline", "edge", "glow", "width"]
        case .interior: ["inside", "bubbles", "period", "angle", "multiplier", "distance", "interior", "detection", "atom", "stripes", "pattern"]
        case .overlays: ["orbit", "map", "mini-map", "minimap", "breadcrumb", "julia", "twin", "fatou", "connected", "dust", "hud", "status", "iterates", "overlay", "cosmic", "scale", "size", "universe", "planck", "developer", "details", "diagnostics", "gpu", "performance", "reference", "kernel", "stats", "target", "marker", "cockpit", "gear", "speed", "compass", "levels", "corner"]
        case .quality: ["iterations", "auto", "maximum", "cap", "upper limit", "anti-aliasing", "aa", "samples", "preview", "quality"]
        case .controls: ["keys", "keyboard", "shortcuts", "mouse", "trackpad", "help", "gamepad"]
        }
    }

    var footer: String? {
        switch self {
        #if os(iOS)
        case .tours: "A tour walks you through a piece of the maths, step by step: it flies the view, sets the colours and overlays and moves the orbit's point for you. Next and Back on its card step through it, End tour ends it and brings back your view and settings. Look around in between as you like."
        #else
        case .tours: "A tour walks you through a piece of the maths, step by step: it flies the view, sets the colours and overlays and moves the orbit's point for you. → and ← step through it, Esc twice ends it and brings back your view and settings. Look around in between as you like."
        #endif
        case .journey: "A journey flies on its own from landmark to landmark it finds in the maths: minibrots, the embedded Julia sets on the way to them, spirals and branch points. The same seed from the same start gives the same journey. Steering or Q down to zero hands over to cruise, other input pauses it; J or a click resumes, Esc stops. With \"Stop at each find\" it waits at each landmark: J flies on, Z goes deeper here, X somewhere else. Effects apply to the autopilot, journeys, fly-to and recorded flights."
        case .look: "A look sets palette, lighting, outline and interior colour together. Colours cycle only while the view rests; depth tint shifts the palette as you zoom."
        case .lighting: "Lighting, outline and glow use the distance estimate (a little GPU work per iteration)."
        case .interior: "Interior colourings analyse each interior pixel's attracting cycle on the GPU (period, multiplier, distance) at any depth; switching between them recolours instantly, switching from flat renders the view again."
        case .overlays: "The orbit follows the pointer (the crosshair in keyboard mode): c as a ring, then z₁…zₙ. Click a mini-map to jump there at the current zoom. The Julia panel shows the Julia set of the orbit's point; click it to open that set, or open the Julia twin of the view (⇧U). The size scale compares the view with real things. O, M, U and C toggle them. The target marker rings where the autopilot or a journey is heading. The HUD shows the zoom, the centre and the speed, and the cockpit at the bottom the driving level (M manual, C cruise, A autopilot, J journey), the speed gauge and lane assist; its developer details add the renderer's statistics."
        case .quality: "While you move, each frame renders as much as fits in a refresh; at rest the view refines and gathers anti-aliasing samples near the boundary."
        default: nil
        }
    }
}

/// Places: the catalogue of famous views (Rust core `places()`, kb: synthesis/places) as a
/// thumbnail grid by category. Selecting one shows what it is in the pinned `PlaceDetailsBar`
/// (Go jumps there, Fly flies there: the fly-to path); Random picks one (of the shown category).
/// While the control center searches and place names match, the grid shows those places of
/// every fractal instead (no category picker; another fractal's tile is labelled with it, and
/// going there switches the fractal: Rust's `go_to_place` takes the place's formula).
///
/// Keyboard (Mac, iPad with a keyboard): selecting a tile puts the keyboard focus on it, and a
/// focused tile is the selection, so Tab / ⇧Tab and the arrow keys step through the places
/// with their details in the bar; Return goes there.
struct PlacesSection: View {
    let model: ExplorerModel
    @Bindable var selection: PlaceSelection
    /// The control center's search query ("": not searching).
    var query = ""
    @FocusState private var focus: String?
    @Environment(\.controlRowWidth) private var rowWidth

    private var category: PlaceCategory? { selection.category }
    private var selected: String? { selection.id }

    private var categories: [(PlaceCategory, String)] {
        var seen: [(PlaceCategory, String)] = []
        for p in model.currentPlaces where !seen.contains(where: { $0.0 == p.category }) {
            seen.append((p.category, p.categoryName))
        }
        return seen
    }

    private var shown: [PlaceInfo] {
        model.currentPlaces.filter { category == nil || $0.category == category }
    }

    private static let minWidth: CGFloat = 88
    private static let spacing: CGFloat = 6

    var body: some View {
        let _ = UIPerf.body("PlacesSection")
        let _ = UIPerf.placesAppeared()
        // Name matches of every fractal while searching; none: the usual grid (the section
        // matched by its title or keywords).
        let results = query.isEmpty ? [] : ControlSearch.places(ExplorerModel.allPlaces, query: query)
        let searching = !results.isEmpty
        if !searching {
            ControlRows.picker("Category", selection: $selection.category) {
                Text("All (\(model.currentPlaces.count))").tag(PlaceCategory?.none)
                ForEach(categories, id: \.1) { c in
                    Text(c.1).tag(PlaceCategory?.some(c.0))
                }
            }
        }
        // The HUD's zoom at each place in this window (place zooms count from the whole set).
        let zoomOffset = model.placeZoomOffset
        let formula = model.formula
        // (One snapshot for the ids and the tiles: `shown` follows the model, and a tile updated
        // after a formula change must not index the new, shorter list with an old index.)
        let list = searching ? results : shown
        let ids = list.map(\.id)
        TileRows(ids: ids, minWidth: Self.minWidth, spacing: Self.spacing, aspect: 1.6) { i in
            let p = list[i]
            let label = searching
                ? ControlSearch.formulaLabel(p.formula, current: formula, options: ExplorerModel.formulaOptions) : nil
            tile(p, zoomLog10: p.zoomLog10 + zoomOffset, formulaLabel: label)
                .modifier(TileKeyStop())
                .focused($focus, equals: p.id)
                // (The selection border is the focus ring: a focused tile is the selected one.)
                .focusEffectDisabled()
                .onKeyPress(phases: .down) { press in key(press, at: i, ids: ids) }
                .onDisappear { model.tileLostKeyFocus("place:" + p.id) }
                .modifier(DemoTileFrame(id: "place:" + p.id))
        }
        .onChange(of: focus) {
            // A focused tile is the selection (Tab, arrows); the canvas leaves it the keys.
            if let focus, focus != selection.id { selection.id = focus }
            model.tileKeyFocus(focus.map { "place:" + $0 }, kind: "place:")
        }
        // The selected place's name, description and Go / Fly / Tour are in the pinned
        // `PlaceDetailsBar` (under the grid they were off screen for every tile near the top).
        if !searching {
            HStack {
                Spacer()
                PlaceRandomButton(model: model, selection: selection)
            }
            #if os(iOS)
            .buttonStyle(.borderless)
            #endif
        }
    }

    /// Selects a place and puts the keyboard focus on its tile (Tab goes on from there; a click
    /// on a button does not move the focus by itself).
    private func choose(_ id: String) {
        selection.id = id
        focus = id
    }

    /// Return goes to the focused place; the arrows move the focus (a row up or down).
    private func key(_ press: KeyPress, at i: Int, ids: [String]) -> KeyPress.Result {
        guard i < ids.count else { return .ignored }
        let (dx, dy): (Int, Int)
        switch press.key {
        case .return:
            selection.id = nil
            model.goToPlace(ids[i])
            return .handled
        case .space:
            choose(ids[i])
            return .handled
        case .leftArrow: (dx, dy) = (-1, 0)
        case .rightArrow: (dx, dy) = (1, 0)
        case .upArrow: (dx, dy) = (0, -1)
        case .downArrow: (dx, dy) = (0, 1)
        default: return .ignored
        }
        let columns = TileGrid.columns(width: rowWidth, minWidth: Self.minWidth, spacing: Self.spacing)
        if let n = ControlSearch.move(from: i, count: ids.count, columns: columns, dx: dx, dy: dy) {
            choose(ids[n])
        }
        return .handled
    }

    @ViewBuilder
    private func tile(_ p: PlaceInfo, zoomLog10: Double, formulaLabel: String?) -> some View {
        #if os(iOS)
        // Touch: the first tap selects and puts the name, Go and Fly on the picture itself
        // (the pinned details bar has the description); a second tap goes there. Going or
        // flying deselects it, so the buttons leave the picture.
        PlaceTouchTile(place: p, zoomLog10: zoomLog10, isSelected: selected == p.id,
                       formulaLabel: formulaLabel,
                       select: { choose(p.id) },
                       go: { selection.id = nil; model.goToPlace(p.id) },
                       fly: { selection.id = nil; model.flyToPlace(p.id) },
                       tour: ExplorerModel.tour(forPlace: p.id).map { t in { selection.id = nil; model.startTour(t.id) } })
        #else
        Button {
            choose(p.id)
        } label: {
            PlaceThumbnail(id: p.id)
                .aspectRatio(1.6, contentMode: .fit)
                .overlay(alignment: .bottomTrailing) { PlaceZoomPill(zoomLog10: zoomLog10) }
                .overlay(alignment: .topLeading) {
                    if ExplorerModel.tour(forPlace: p.id) != nil { PlaceTourBadge() }
                }
                .overlay(alignment: .topTrailing) {
                    if let formulaLabel { PlaceFormulaLabel(text: formulaLabel) }
                }
                .clipShape(RoundedRectangle(cornerRadius: 5, style: .continuous))
                // Only the selected tile has a border (every shape is one more layer to
                // build when the section opens).
                .overlay {
                    if selected == p.id {
                        RoundedRectangle(cornerRadius: 5, style: .continuous).strokeBorder(Color.accentColor, lineWidth: 2)
                    }
                }
        }
        .buttonStyle(.plain)
        // The tile (`.focusable()` around it) is the one keyboard stop, not the button too
        // (with full keyboard access a button is a stop of its own: Tab stayed on the tile).
        .focusable(false)
        // The name on hover (the tile shows only the zoom).
        .help(formulaLabel.map { "\(p.name) · \($0)" } ?? p.name)
        .accessibilityLabel(p.name)
        .simultaneousGesture(TapGesture(count: 2).onEnded { model.goToPlace(p.id) })
        #endif
    }
}

/// A grid tile as one keyboard stop. Mac: `focusable` (a stop whether or not keyboard navigation
/// is on in System Settings; the tile's button is no stop of its own). iOS: the tile's button
/// itself, which iPadOS keyboard navigation reaches (no wrapper around the touch tile, whose
/// taps and Go / Fly buttons stay exactly as they were).
struct TileKeyStop: ViewModifier {
    func body(content: Content) -> some View {
        #if os(macOS)
        content.focusable()
        #else
        content
        #endif
    }
}

/// `--ui-demo=tile-focus` only: records a tile's frame in the window (`UIDemo.tileFrames`), so
/// the check clicks it with a real event. Nothing in any other run.
struct DemoTileFrame: ViewModifier {
    let id: String

    func body(content: Content) -> some View {
        if UIDemo.recordsTileFrames {
            content.onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { UIDemo.tileFrames[id] = $0 }
        } else {
            content
        }
    }
}

/// `--ui-demo=place-details` only: records the place the pinned details bar shows
/// (`UIDemo.shownDetails`). Nothing in any other run.
struct DemoShownDetails: ViewModifier {
    let id: String

    func body(content: Content) -> some View {
        if UIDemo.recordsTileFrames {
            content
                .task(id: id) { UIDemo.shownDetails = id }
                .onDisappear { UIDemo.shownDetails = nil }
        } else {
            content
        }
    }
}

/// The fractal of a search result from another fractal than the current one ("Burning Ship",
/// "z⁸"), top right on its tile.
struct PlaceFormulaLabel: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.system(size: 9, weight: .semibold))
            .lineLimit(1)
            .foregroundStyle(.white)
            .padding(.horizontal, 4)
            .padding(.vertical, 1.5)
            .background(.black.opacity(0.6), in: RoundedRectangle(cornerRadius: 3.5, style: .continuous))
            .padding(3)
            .accessibilityHidden(true)
    }
}

/// The Places grid's selection and category, shared by the grid (`PlacesSection`) and the
/// pinned details bar (`PlaceDetailsBar`). One object per control center: the sections keep
/// equal inputs, and only the views that read `id` follow a new selection (no per-tile cost).
@Observable
@MainActor
final class PlaceSelection {
    /// The selected place's id; nil: none (the bar hides).
    var id: String?
    /// The grid's category filter (Random picks from it).
    var category: PlaceCategory?
    /// Whether enough of the Places section is inside the scroll view's visible area (Mac;
    /// `PlacesOnScreen`): scrolled down to other sections, the bar gets out of the way.
    var onScreen = true

    init(id: String? = nil) { self.id = id }
}

/// Mac: keeps `PlaceSelection.onScreen` up to date from the Places card's frame in the scroll
/// view (one geometry reader on the card, not per tile; its value is a Bool, so the action runs
/// only when the section scrolls in or out). iOS's `Form` is lazy: the section's open state alone
/// decides there.
struct PlacesOnScreen: ViewModifier {
    let selection: PlaceSelection
    let active: Bool

    func body(content: Content) -> some View {
        if active {
            content
                .onGeometryChange(for: Bool.self) { proxy in
                    // At least ~two tile rows of the section in view, or half of a shorter card:
                    // a search with one row of results makes the whole card ~70 pt, and a fixed
                    // 90 pt hid the bar for every place selected there.
                    guard let visible = proxy.bounds(of: .scrollView) else { return true }
                    let need = min(90, proxy.size.height / 2)
                    return CGRect(origin: .zero, size: proxy.size).intersection(visible).height > need
                } action: { selection.onScreen = $0 }
                .onDisappear { selection.onScreen = true }
                .modifier(DemoTileFrame(id: "card:places"))
        } else {
            content
        }
    }
}

/// Random: selects (and jumps to) a random place of the shown category.
struct PlaceRandomButton: View {
    let model: ExplorerModel
    let selection: PlaceSelection
    var iconOnly = false

    var body: some View {
        Button {
            selection.id = model.randomPlace(category: selection.category).id
        } label: {
            if iconOnly {
                Image(systemName: "dice")
            } else {
                Label("Random", systemImage: "dice")
            }
        }
        .help("A random place\(selection.category == nil ? "" : " of this category")")
        .accessibilityLabel("Random place")
    }
}

/// The selected place, pinned to the bottom of the control center (Mac: above Reset all) while the
/// Places section is open (and, on the Mac, in view) and a place is selected: name, category ·
/// zoom · feature, the description (a few lines, the rest scrolls) and, on the Mac, Go / Fly /
/// Tour / Random. ✕ clears the selection. Touch has Go / Fly / Tour on the selected tile itself,
/// so the bar only adds what the tile cannot show, and replaces the footer (Reset all) while it
/// shows: on the iPhone's half sheet every point goes to the grid (kb: decisions/controls "Places").
struct PlaceDetailsBar<Footer: View>: View {
    let model: ExplorerModel
    let selection: PlaceSelection
    let sectionOpen: Bool
    @ViewBuilder let footer: Footer

    var body: some View {
        let place = sectionOpen && selection.onScreen
            ? selection.id.flatMap { id in ExplorerModel.allPlaces.first { $0.id == id } } : nil
        VStack(spacing: 0) {
            if let place { details(place).modifier(DemoShownDetails(id: place.id)) }
            #if os(iOS)
            if place == nil { footer }
            #else
            footer
            #endif
        }
    }

    private func details(_ p: PlaceInfo) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(alignment: .firstTextBaseline, spacing: 6) {
                Text(p.name)
                    .font(.callout.weight(.semibold))
                    .lineLimit(1)
                    .truncationMode(.tail)
                Spacer(minLength: 4)
                Button {
                    selection.id = nil
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.secondary)
                        #if os(iOS)
                        .font(.body)
                        .frame(minWidth: 32, minHeight: 28)
                        .contentShape(Rectangle())
                        #endif
                }
                .buttonStyle(.plain)
                .help("Clear the selection")
                .accessibilityLabel("Clear the place selection")
            }
            Text("\(p.categoryName) · \(Format.zoom(log10: p.zoomLog10 + model.placeZoomOffset)) · \(p.feature)")
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(Self.metaLines)
            CappedText(text: p.description, maxHeight: Self.descriptionCap)
            #if os(macOS)
            HStack {
                Button {
                    model.goToPlace(p.id)
                } label: {
                    Label("Go", systemImage: "arrow.right.circle")
                }
                .help("Jump there (double-click a thumbnail)")
                Button {
                    model.flyToPlace(p.id)
                } label: {
                    Label("Fly", systemImage: "airplane")
                }
                .help("Fly there: out, across and in; Esc stops")
                // A place with a guided tour (kb: decisions/controls "Guided tours").
                if let tour = ExplorerModel.tour(forPlace: p.id) {
                    Button {
                        model.startTour(tour.id)
                    } label: {
                        Label("Tour", systemImage: "signpost.right")
                    }
                    .help("Take the guided tour “\(tour.title)”: → and ← step through it, Esc twice ends it")
                }
                Spacer()
                PlaceRandomButton(model: model, selection: selection, iconOnly: true)
            }
            .padding(.top, 2)
            #endif
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.bar)
        .overlay(alignment: .top) { Divider() }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("placeDetails")
    }

    /// The description's height before it scrolls: five caption lines on the Mac (the longest
    /// description fits the default width), two and a half on touch (the iPhone's half sheet
    /// keeps its height for the grid; the cut line says there is more to scroll).
    #if os(macOS)
    static var descriptionCap: CGFloat { 66 }
    static var metaLines: Int { 2 }
    #else
    static var descriptionCap: CGFloat { 40 }
    static var metaLines: Int { 1 }
    #endif
}

/// A text as tall as it is up to `maxHeight`, then scrolling (the full text stays reachable).
struct CappedText: View {
    let text: String
    let maxHeight: CGFloat
    @State private var height: CGFloat = 0

    var body: some View {
        ScrollView {
            Text(text)
                .font(.caption)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
                .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { height = $0 }
        }
        // A new text starts at its top.
        .id(text)
        .scrollBounceBehavior(.basedOnSize)
        .frame(height: min(max(height, 14), maxHeight))
    }
}

#if os(iOS)
/// A place's tile on touch (kb: decisions/controls "Touch", places grid). No hover, and the
/// grid is long, so the actions are on the picture (the pinned `PlaceDetailsBar` has the
/// description): the first tap selects it (a border, the name along the bottom edge, Go and Fly over the image); a tap on
/// the selected picture outside the buttons goes there; another tile moves the selection. A
/// long press offers Go / Fly too; VoiceOver has them as the tile's actions.
struct PlaceTouchTile: View {
    let place: PlaceInfo
    let zoomLog10: Double
    let isSelected: Bool
    /// A search result's fractal when it is not the current one ("Burning Ship", "z⁸").
    var formulaLabel: String? = nil
    let select: () -> Void
    let go: () -> Void
    let fly: () -> Void
    /// Starts the place's guided tour (a tour names it in `place:`); nil without one.
    var tour: (() -> Void)?

    /// The name strip along the bottom edge of the selected tile.
    private static let nameHeight: CGFloat = 16

    var body: some View {
        Button {
            if isSelected { go() } else { select() }
        } label: {
            PlaceThumbnail(id: place.id)
                .aspectRatio(1.6, contentMode: .fit)
                .overlay(alignment: .bottomTrailing) {
                    if !isSelected { PlaceZoomPill(zoomLog10: zoomLog10) }
                }
                // A place with a tour says so (kb: decisions/controls "Finding the tours").
                .overlay(alignment: .topLeading) {
                    if tour != nil && !isSelected { PlaceTourBadge() }
                }
                .overlay(alignment: .topTrailing) {
                    if let formulaLabel, !isSelected { PlaceFormulaLabel(text: formulaLabel) }
                }
                .overlay(alignment: .bottom) {
                    if isSelected {
                        Text(place.name)
                            .font(.caption2.weight(.semibold))
                            .lineLimit(1)
                            .minimumScaleFactor(0.75)
                            .foregroundStyle(.white)
                            .padding(.horizontal, 4)
                            .frame(maxWidth: .infinity, minHeight: Self.nameHeight)
                            .background(.black.opacity(0.65))
                    }
                }
                .clipShape(RoundedRectangle(cornerRadius: 5, style: .continuous))
                // Only the selected tile has a border (every shape is one more layer to
                // build when the section opens).
                .overlay {
                    if isSelected {
                        RoundedRectangle(cornerRadius: 5, style: .continuous).strokeBorder(Color.accentColor, lineWidth: 3)
                    }
                }
        }
        .buttonStyle(.plain)
        .accessibilityLabel(place.name)
        .accessibilityHint(isSelected ? "Tap again to go there" : "Selects the place")
        .accessibilityAddTraits(isSelected ? .isSelected : [])
        .accessibilityAction(named: "Go") { go() }
        .accessibilityAction(named: "Fly") { fly() }
        .accessibilityAction(named: "Tour") { tour?() }
        .contextMenu {
            Section(place.name) {
                Button(action: go) { Label("Go", systemImage: "arrow.right.circle") }
                Button(action: fly) { Label("Fly", systemImage: "airplane") }
                if let tour { Button(action: tour) { Label("Tour", systemImage: "signpost.right") } }
            }
        }
        .overlay {
            if isSelected {
                VStack(spacing: 0) {
                    // Icon and label side by side where they fit, else the label under the icon.
                    // With a tour, Tour is the third button: labelled where it fits, else icons
                    // only (narrower), else only Go and Fly (the long press still has Tour).
                    ViewThatFits {
                        if tour != nil {
                            actions(stacked: false, withTour: true)
                            actions(stacked: true, withTour: true)
                            actions(iconsOnly: true, withTour: true)
                        }
                        actions(stacked: false)
                        actions(stacked: true)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    Color.clear.frame(height: Self.nameHeight)
                }
            }
        }
    }

    private func actions(stacked: Bool = false, iconsOnly: Bool = false, withTour: Bool = false) -> some View {
        HStack(spacing: iconsOnly ? 2 : 4) {
            action("Go", "arrow.right.circle", id: "placeTileGo", stacked: stacked, iconsOnly: iconsOnly, go)
            action("Fly", "airplane", id: "placeTileFly", stacked: stacked, iconsOnly: iconsOnly, fly)
            if withTour, let tour {
                action("Tour", "signpost.right", id: "placeTileTour", stacked: stacked, iconsOnly: iconsOnly, tour)
            }
        }
        .padding(.horizontal, iconsOnly ? 2 : 3)
    }

    private func action(_ title: String, _ icon: String, id: String, stacked: Bool, iconsOnly: Bool = false,
                        _ perform: @escaping () -> Void) -> some View {
        Button(action: perform) {
            Group {
                if iconsOnly {
                    Image(systemName: icon).font(.system(size: 14, weight: .semibold))
                } else if stacked {
                    VStack(spacing: 1) {
                        Image(systemName: icon).font(.system(size: 13, weight: .semibold))
                        Text(title).font(.system(size: 10, weight: .semibold))
                    }
                    .padding(.horizontal, 5)
                } else {
                    HStack(spacing: 4) {
                        Image(systemName: icon)
                        Text(title)
                    }
                    .font(.footnote.weight(.semibold))
                    .padding(.horizontal, 10)
                }
            }
            .lineLimit(1)
            .fixedSize()
            .frame(minWidth: iconsOnly ? 32 : 44, minHeight: 44)
            .foregroundStyle(.primary)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .contentShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
        }
        // Not the Form row's default style: that fires every button in the row on a tap.
        .buttonStyle(.plain)
        .accessibilityLabel("\(title) to \(place.name)")
        .accessibilityIdentifier(id)
    }
}
#endif

/// A small signpost in the corner of a place's tile when a guided tour starts there.
struct PlaceTourBadge: View {
    var body: some View {
        Image(systemName: "signpost.right.fill")
            .font(.system(size: 9, weight: .bold))
            .foregroundStyle(.white)
            .frame(width: 18, height: 18)
            .background(.black.opacity(0.6), in: Circle())
            .padding(3)
            .accessibilityHidden(true)
    }
}

/// The place's zoom in a small dark pill at the bottom right of its thumbnail (the name is
/// the tooltip). Plain text: nothing changes per frame.
struct PlaceZoomPill: View {
    let zoomLog10: Double

    var body: some View {
        Text(Format.zoom(log10: zoomLog10))
            .font(.system(size: 9, weight: .semibold))
            .monospacedDigit()
            .lineLimit(1)
            .foregroundStyle(.white)
            .padding(.horizontal, 4)
            .padding(.vertical, 1.5)
            .background(.black.opacity(0.6), in: RoundedRectangle(cornerRadius: 3.5, style: .continuous))
            .padding(3)
            .accessibilityHidden(true)
    }
}

/// A place's thumbnail from the pre-decoded cache (PlaceThumbnails.preload at launch); if the
/// launch decode has not reached it yet, it waits for it off the main thread.
struct PlaceThumbnail: View {
    let id: String
    @State private var image: CGImage?
    @Environment(\.displayScale) private var displayScale

    var body: some View {
        let _ = UIPerf.body("PlaceThumbnail")
        let shown = image ?? PlaceThumbnails.cached(id)
        if let shown {
            let _ = UIPerf.thumbnailVisible(id, immediate: image == nil)
            Image(decorative: shown, scale: displayScale).resizable()
        } else {
            // Only a tile without its image waits (no task per tile when the cache has it).
            Rectangle().fill(.quaternary)
                .task(id: id) { image = await PlaceThumbnails.decoded(id) }
        }
    }
}

/// "Import palette…": .ugr / .map / JSON files into "My palettes" (kept across launches); the
/// grid then shows My palettes. Behind Look & colour's More.
struct PaletteImportRow: View {
    let model: ExplorerModel
    @AppStorage("palettes.category") private var category = ""
    @State private var importing = false
    @State private var importNote: String?

    var body: some View {
        HStack {
            Button {
                importing = true
            } label: {
                Label("Import palette…", systemImage: "square.and.arrow.down")
            }
            .help("Ultra Fractal .ugr, Fractint .map or almondbread JSON")
            if let importNote {
                Text(importNote).font(.caption).foregroundStyle(.secondary).lineLimit(2)
            }
        }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.data, .json, .plainText],
                      allowsMultipleSelection: true) { result in
            switch result {
            case .success(let urls):
                var n = 0
                var failures: [String] = []
                for url in urls {
                    do { n += try model.importPalettes(from: url) } catch { failures.append("\(error)") }
                }
                importNote = failures.isEmpty ? "Imported \(n) palette\(n == 1 ? "" : "s") into My palettes"
                    : "Imported \(n); failed: \(failures.joined(separator: "; "))"
                if n > 0 { category = "user" }
            case .failure(let e):
                importNote = "\(e.localizedDescription)"
            }
        }
    }
}

/// The palette picker: the palette library's swatches in a grid by category (separate from
/// the look, which is a colouring preset). Choosing one applies its recommended density,
/// offset and deep colour; "Import palette…" reads .ugr / .map / JSON files into "My
/// palettes" (kept across launches). While the control center searches and palette names
/// match, the grid shows those palettes (no category picker). Import: `PaletteImportRow`. Keyboard: a chosen swatch takes
/// the keyboard focus, so Tab / ⇧Tab and the arrows go on from it; Space or Return applies the
/// focused one.
struct PalettePicker: View {
    @Bindable var model: ExplorerModel
    /// The control center's search query ("": not searching).
    var query = ""
    @AppStorage("palettes.category") private var category = ""
    @FocusState private var focus: String?
    @Environment(\.controlRowWidth) private var rowWidth

    private static let minWidth: CGFloat = 64
    private static let spacing: CGFloat = 6


    private var categories: [(key: String, label: String)] {
        var seen: [(key: String, label: String)] = []
        for p in model.library where !seen.contains(where: { $0.key == p.category }) {
            seen.append((p.category, p.categoryLabel))
        }
        return seen
    }

    var body: some View {
        let _ = UIPerf.body("PalettePicker")
        // The Mac: header, category, each grid row and the import button are rows of their own
        // (card rows, kb: platform/swiftui-performance); iOS: one form row.
        #if os(macOS)
        parts
        #else
        VStack(alignment: .leading, spacing: 6) { parts }
        #endif
    }

    @ViewBuilder private var parts: some View {
            HStack {
                Text("Palette")
                Spacer()
                Text(model.palette.name).foregroundStyle(.secondary)
            }
            let results = query.isEmpty ? [] : ControlSearch.palettes(model.library, query: query)
            let searching = !results.isEmpty
            if !searching {
                Picker("Category", selection: $category) {
                    Text("All (\(model.library.count))").tag("")
                    ForEach(categories, id: \.key) { c in Text(c.label).tag(c.key) }
                }
                .labelsHidden()
            }
            let shown = searching ? results : model.library.filter { category.isEmpty || $0.category == category }
            let ids = shown.map(\.id)
            TileRows(ids: ids, minWidth: Self.minWidth, spacing: Self.spacing, height: 16) { i in
                let p = shown[i]
                Button {
                    choose(p.id)
                } label: {
                    PaletteSwatch(palette: PaletteInfo(id: p.id, name: p.name, stops: p.stops))
                        .frame(height: 16)
                        .overlay {
                            if model.paletteID == p.id {
                                RoundedRectangle(cornerRadius: 3, style: .continuous).strokeBorder(Color.accentColor, lineWidth: 2)
                            }
                        }
                }
                .buttonStyle(.plain)
                // Mac: one keyboard stop per swatch (`TileKeyStop` below), not the button too.
                #if os(macOS)
                .focusable(false)
                #endif
                .help("\(p.name) · \(p.categoryLabel)\(p.attribution.isEmpty ? "" : " · \(p.attribution)")")
                .accessibilityLabel(p.name)
                .accessibilityAddTraits(model.paletteID == p.id ? .isSelected : [])
                .modifier(TileKeyStop())
                .focused($focus, equals: p.id)
                .onKeyPress(phases: .down) { press in key(press, at: i, ids: ids) }
                .onDisappear { model.tileLostKeyFocus("palette:" + p.id) }
                .modifier(DemoTileFrame(id: "palette:" + p.id))
            }
            .onChange(of: focus) { model.tileKeyFocus(focus.map { "palette:" + $0 }, kind: "palette:") }
    }

    /// Applies a palette and puts the keyboard focus on its swatch (Tab goes on from there).
    private func choose(_ id: String) {
        model.applyPalette(id)
        focus = id
    }

    /// Space or Return applies the focused swatch; the arrows move the focus (a row up or down).
    private func key(_ press: KeyPress, at i: Int, ids: [String]) -> KeyPress.Result {
        guard i < ids.count else { return .ignored }
        let (dx, dy): (Int, Int)
        switch press.key {
        case .return, .space:
            model.applyPalette(ids[i])
            return .handled
        case .leftArrow: (dx, dy) = (-1, 0)
        case .rightArrow: (dx, dy) = (1, 0)
        case .upArrow: (dx, dy) = (0, -1)
        case .downArrow: (dx, dy) = (0, 1)
        default: return .ignored
        }
        let columns = TileGrid.columns(width: rowWidth, minWidth: Self.minWidth, spacing: Self.spacing)
        if let n = ControlSearch.move(from: i, count: ids.count, columns: columns, dx: dx, dy: dy) {
            focus = ids[n]
        }
        return .handled
    }
}

/// Keys and gestures.
struct ControlsHelp: View {
    let model: ExplorerModel

    var body: some View {
        #if os(macOS)
        row("scope", "Move the mouse", "Pointer mode: zoom at the cursor")
        row("keyboard", "Hold W A S D / arrows", "Pan; keyboard mode: zoom at the centre")
        row("space", "Hold Space / ⇧Space", "Zoom in / out at the crosshair")
        row("arrow.up.left.and.arrow.down.right", "Pinch, ⌘/⌥-scroll or wheel", "Zoom at the crosshair")
        row("plus.magnifyingglass", "+  −", "Zoom step at the crosshair")
        row("hand.draw", "Two-finger scroll or drag", "Pan")
        row("cursorarrow.click.2", "Double-click / ⌥ double-click", "Zoom in / out there")
        row("arrow.up.and.down.and.arrow.left.and.right", "Pointer at an edge", "Pan that way (while zooming)")
        row("gauge.with.dots.needle.33percent", "E / Q", "Cruise: one step in / out per press, hold for more; to zero and stop")
        row("space", "Space held + Q / E", "Space zoom speed − / + (\(Format.speed(model.zoomFactorPerSecond)))")
        row("road.lanes", "L", "Lane assist in cruise: keeps you out of featureless areas")
        row("point.topleft.down.to.point.bottomright.curvepath", "O", "Orbit overlay")
        row("map", "M · click a map · ⌥-click", "Mini-map · jump there · fly there")
        row("seal", "U", "Julia set of the orbit's point; click it to open that set")
        row("seal", "⌘-click a point", "Open that point's Julia set")
        row("seal", "⇧U", "Julia twin of the view (its centre's Julia set) / back")
        row("airplane", "Z", "Autopilot; steering hands over to cruise, click pauses")
        row("location.north.line", "⌘G / ⇧⌘G", "Fly to coordinates / copied ones")
        row("arrow.uturn.backward", "⌘Z / ⇧⌘Z or ⌘Y", "Back to the places you visited / forward again")
        row("sparkles", "J  /  ⇧J", "Journey: on · resume · off  /  in the other mode")
        row("forward", "At a journey stop", "J next · Z deeper here · X somewhere else")
        row("signpost.right", "In a tour  → / ← / Esc", "Next step / back / end the tour")
        row("rotate.left", ",  .  /", "Turn left / right · upright (or trackpad rotate)")
        row("escape", "Esc", "Stop the cruise, the autopilot, a flight or a journey; then leave full screen")
        row("arrow.up.backward.and.arrow.down.forward", "F or ⌃⌘F", "Full screen on / off")
        row("arrow.counterclockwise", "R or ⌘0", "Reset view")
        row("sidebar.trailing", "⌘I", "Control center")
        row("gearshape", "⌘,", "Settings")
        #else
        row("hand.draw", "Drag", "Pan")
        row("arrow.up.left.and.arrow.down.right", "Pinch", "Zoom at the fingers, and pan")
        row("hand.tap", "Double tap", "Zoom in")
        row("hand.tap", "Two-finger tap", "Zoom out")
        row("map", "Tap / hold a mini-map", "Jump there / fly there")
        row("scope", "Trackpad or mouse pointer", "Zoom at the pointer")
        row("keyboard", "Keyboard: W A S D / arrows", "Pan; zoom at the centre")
        row("space", "Keyboard: Space / ⇧Space", "Zoom")
        row("gauge.with.dots.needle.33percent", "Keyboard: E / Q, L, N, G", "Cruise in / out, lane assist, seek minibrot, continue")
        row("map", "Keyboard: O / M / U", "Orbit overlay / mini-map / Julia set")
        row("seal", "Tap the Julia panel · ⇧U", "Open that Julia set · Julia twin / back")
        // (iPad only: a ⌘-click needs a keyboard and a pointer; no ⌘ hint on phones.)
        if UIDevice.current.userInterfaceIdiom == .pad {
            row("seal", "Keyboard: ⌘-click a point", "Open that point's Julia set (trackpad or mouse)")
        }
        row("gauge.with.dots.needle.33percent", "Throttle (screen edge)", "The knob is the gear: up in, down out, middle stops; tap a notch or a chevron; double tap stops")
        row("steeringwheel", "Cockpit icons", "Tap: manual, cruise, autopilot, journey, lane assist")
        row("hand.point.up.left", "Long press (nothing flying)", "Seek minibrot, orbit, fly here, copy; drag moves the orbit")
        row("circle.circle", "Round buttons", "Orbit, mini-map, Julia set (iPad), size scale on / off; fly to; control center")
        row("airplane", "Cockpit plane or Z", "Flies by itself; a resting finger suggests a direction, a tap pauses")
        row("hand.point.up.left", "Rest a finger while flying", "Steer towards it while it stays down (a journey hands over to cruise)")
        row("hand.draw", "Swipe while flying", "Move the picture; the zoom goes on")
        row("sparkles", "Cockpit sparkles or J", "Landmark to landmark; a tap pauses")
        row("forward", "At a journey stop", "Buttons under the status: next · deeper here · elsewhere")
        row("scope", "Seek minibrot", "Long press (here) or the control center")
        row("arrow.uturn.backward", "Back / Forward", "The places you visited: control center or long press (keyboard: ⌘Z / ⇧⌘Z)")
        row("rotate.left", "Two-finger rotate", "Turn the view")
        #endif
    }

    private func row(_ symbol: String, _ input: String, _ action: String) -> some View {
        HStack(spacing: 8) {
            Image(systemName: symbol)
                .frame(width: 20)
                .foregroundStyle(.secondary)
            Text(input)
            Spacer()
            Text(action).foregroundStyle(.secondary).multilineTextAlignment(.trailing)
        }
        .font(.caption)
        .accessibilityElement(children: .combine)
    }
}

/// The palette's gradient as a small horizontal bar.
struct PaletteSwatch: View {
    let palette: PaletteInfo

    var body: some View {
        RoundedRectangle(cornerRadius: 3, style: .continuous)
            .fill(LinearGradient(gradient: palette.swatchGradient, startPoint: .leading, endPoint: .trailing))
            .overlay(RoundedRectangle(cornerRadius: 3, style: .continuous).strokeBorder(.white.opacity(0.15)))
    }
}

extension PaletteInfo {
    /// One full cycle, closed back to the first stop like the GPU table.
    var swatchGradient: Gradient {
        func color(_ hex: UInt32) -> Color {
            Color(.sRGB, red: Double((hex >> 16) & 0xFF) / 255, green: Double((hex >> 8) & 0xFF) / 255,
                  blue: Double(hex & 0xFF) / 255)
        }
        var stops = self.stops.map { Gradient.Stop(color: color($0.rgb), location: $0.position) }
        if let first = self.stops.first {
            stops.append(Gradient.Stop(color: color(first.rgb), location: 1))
        }
        return Gradient(stops: stops)
    }
}

#if os(macOS)
/// App-wide preferences (⌘,): defaults that apply to every window and launch.
public struct AppSettingsView: View {
    @AppStorage(Preferences.edgePanKey) private var edgePanning = EdgePanning.whileZooming.rawValue
    @AppStorage(Preferences.gamepadKey) private var gamepad = true
    @AppStorage(Preferences.zoomUnitsKey) private var zoomUnits = ZoomUnits.magnification.rawValue
    @AppStorage(Preferences.restoreKey) private var restore = true
    @AppStorage(Preferences.hudKey) private var showHUD = true
    @AppStorage(Preferences.hudDetailsKey) private var hudDetails = false
    @FocusedValue(\.explorerModel) private var model

    public init() {}

    public var body: some View {
        TabView {
            Form {
                Toggle("Restore colours and overlays at launch", isOn: $restore)
                Toggle("Show the status overlay (HUD)", isOn: $showHUD)
                Toggle("Developer details in the status overlay", isOn: $hudDetails)
                    .disabled(!showHUD)
                Picker("Zoom shown as", selection: $zoomUnits) {
                    ForEach(ZoomUnits.allCases) { Text($0.label).tag($0.rawValue) }
                }
                Button("Reset colours, interior and overlays to the defaults") {
                    if let model {
                        model.resetSettings()
                    } else {
                        UserDefaults.standard.removeObject(forKey: Preferences.savedSettingsKey)
                    }
                }
            }
            .formStyle(.grouped)
            .tabItem { Label("General", systemImage: "gearshape") }

            Form {
                Picker("Edge panning", selection: $edgePanning) {
                    ForEach(EdgePanning.allCases) { Text($0.label).tag($0.rawValue) }
                }
                Text("With the pointer near an edge, the view also pans towards it. “While zooming” pans only while Space is held.")
                    .font(.caption).foregroundStyle(.secondary)
                Toggle("Game controllers", isOn: $gamepad)
                Text("Left stick pans, triggers zoom, A boosts, shoulders are the cruise pedals (Q / E), Y resets. Takes effect in new windows.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            .formStyle(.grouped)
            .tabItem { Label("Navigation", systemImage: "move.3d") }
        }
        .frame(width: 460, height: 330)
        .onChange(of: edgePanning) {
            if let m = model, let e = EdgePanning(rawValue: edgePanning) { m.edgePanning = e }
        }
        .onChange(of: showHUD) { model?.showHUD = showHUD }
        .onChange(of: hudDetails) { model?.hudDetails = hudDetails }
    }
}
#endif

/// Tiles in adaptive columns, like `LazyVGrid(columns: [GridItem(.adaptive(minimum:), spacing:)])`
/// but with the size computed from the width and the count alone: every tile has the same
/// shape (an aspect ratio or a fixed height), so measuring the grid never asks the tiles. Inside
/// the control center's (non-lazy) form a lazy grid re-estimated its tiles on every layout
/// pass of the sidebar (kb: platform/swiftui-performance).
struct TileGrid: Layout {
    var minWidth: CGFloat
    var spacing: CGFloat
    /// Width over height of a tile, or a fixed `height`.
    var aspect: CGFloat = 1
    var height: CGFloat?
    /// A fixed column count (a row of `TileRows`); nil: as many as fit.
    var columns: Int?

    /// How many tiles of `minWidth` fit in `width` (the arrow keys' row length too).
    static func columns(width: CGFloat, minWidth: CGFloat, spacing: CGFloat) -> Int {
        max(1, Int((width + spacing) / (minWidth + spacing)))
    }

    private func geometry(width: CGFloat) -> (columns: Int, tile: CGSize) {
        let n = columns ?? Self.columns(width: width, minWidth: minWidth, spacing: spacing)
        let w = max(0, (width - spacing * CGFloat(n - 1)) / CGFloat(n))
        return (n, CGSize(width: w, height: height ?? w / aspect))
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        // A stack may propose an infinite width (its ideal-size pass): three columns then.
        let width = proposal.width.flatMap { $0.isFinite ? $0 : nil } ?? (minWidth * 3 + spacing * 2)
        let g = geometry(width: width)
        let rows = (subviews.count + g.columns - 1) / g.columns
        return CGSize(width: width, height: rows == 0 ? 0 : CGFloat(rows) * g.tile.height + CGFloat(rows - 1) * spacing)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let g = geometry(width: bounds.width)
        let tile = ProposedViewSize(g.tile)
        for (i, s) in subviews.enumerated() {
            let x = bounds.minX + CGFloat(i % g.columns) * (g.tile.width + spacing)
            let y = bounds.minY + CGFloat(i / g.columns) * (g.tile.height + spacing)
            s.place(at: CGPoint(x: x, y: y), proposal: tile)
        }
    }
}

