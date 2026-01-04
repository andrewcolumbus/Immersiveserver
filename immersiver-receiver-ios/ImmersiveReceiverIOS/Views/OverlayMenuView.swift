import SwiftUI

struct OverlayMenuView: View {
    @EnvironmentObject var appState: AppState
    @State private var manualHost: String = ""
    @State private var manualPort: String = "9030"
    @State private var showingManualEntry: Bool = false
    
    var body: some View {
        VStack(spacing: 0) {
            // Header
            headerSection
            
            Divider()
                .background(Color.white.opacity(0.2))
            
            ScrollView {
                VStack(spacing: 20) {
                    // Network Info
                    networkInfoSection
                    
                    // Mode Toggle
                    modeSection
                    
                    // Playback Controls
                    playbackSection
                    
                    // Source Selection (only in receiver mode)
                    if appState.mode == .receiver {
                        sourceSection
                    }
                    
                    // Zoom Controls
                    if appState.zoomScale != 1.0 || appState.panOffset != .zero {
                        zoomResetSection
                    }
                }
                .padding()
            }
        }
        .frame(maxWidth: 400, maxHeight: 600)
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 20))
        .overlay(
            RoundedRectangle(cornerRadius: 20)
                .stroke(Color.white.opacity(0.1), lineWidth: 1)
        )
        .shadow(color: .black.opacity(0.3), radius: 20)
        .padding()
    }
    
    // MARK: - Header
    
    private var headerSection: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("Immersive Receiver")
                    .font(.system(size: 18, weight: .semibold, design: .rounded))
                    .foregroundColor(.white)
                
                Text(appState.connectionState.displayText)
                    .font(.system(size: 12, weight: .medium, design: .monospaced))
                    .foregroundColor(.white.opacity(0.6))
            }
            
            Spacer()
            
            Button(action: { appState.toggleMenu() }) {
                Image(systemName: "xmark.circle.fill")
                    .font(.system(size: 24))
                    .foregroundColor(.white.opacity(0.6))
            }
        }
        .padding()
    }
    
    // MARK: - Network Info
    
    private var networkInfoSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("Network", systemImage: "network")
                .font(.system(size: 14, weight: .semibold))
                .foregroundColor(.white.opacity(0.8))
            
            HStack {
                Text("Device IP:")
                    .foregroundColor(.white.opacity(0.6))
                Spacer()
                Text(appState.deviceIPAddress)
                    .font(.system(size: 14, weight: .medium, design: .monospaced))
                    .foregroundColor(.cyan)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
            .background(Color.white.opacity(0.05), in: RoundedRectangle(cornerRadius: 8))
            
            if appState.videoDimensions.width > 0 {
                HStack {
                    Text("Resolution:")
                        .foregroundColor(.white.opacity(0.6))
                    Spacer()
                    Text("\(appState.videoDimensions.width) × \(appState.videoDimensions.height)")
                        .font(.system(size: 14, weight: .medium, design: .monospaced))
                        .foregroundColor(.green)
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 10)
                .background(Color.white.opacity(0.05), in: RoundedRectangle(cornerRadius: 8))
            }
        }
    }
    
    // MARK: - Mode Section
    
    private var modeSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("Mode", systemImage: "arrow.left.arrow.right")
                .font(.system(size: 14, weight: .semibold))
                .foregroundColor(.white.opacity(0.8))
            
            Picker("Mode", selection: Binding(
                get: { appState.mode },
                set: { appState.switchMode($0) }
            )) {
                ForEach(AppMode.allCases, id: \.self) { mode in
                    Text(mode.rawValue).tag(mode)
                }
            }
            .pickerStyle(.segmented)
        }
    }
    
    // MARK: - Playback Section
    
    private var playbackSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("Playback", systemImage: "play.circle")
                .font(.system(size: 14, weight: .semibold))
                .foregroundColor(.white.opacity(0.8))
            
            Button(action: { appState.togglePlayback() }) {
                HStack {
                    Image(systemName: appState.isPlaying ? "stop.fill" : "play.fill")
                        .font(.system(size: 18))
                    
                    Text(playButtonText)
                        .font(.system(size: 16, weight: .medium))
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 14)
                .background(playButtonColor, in: RoundedRectangle(cornerRadius: 10))
                .foregroundColor(.white)
            }
        }
    }
    
    private var playButtonText: String {
        if appState.mode == .receiver {
            return appState.isPlaying ? "Stop Receiving" : "Start Receiving"
        } else {
            return appState.isPlaying ? "Stop Broadcasting" : "Start Broadcasting"
        }
    }
    
    private var playButtonColor: Color {
        appState.isPlaying ? .red.opacity(0.8) : .green.opacity(0.8)
    }
    
    // MARK: - Source Section
    
    private var sourceSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Label("Sources", systemImage: "antenna.radiowaves.left.and.right")
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundColor(.white.opacity(0.8))
                
                Spacer()
                
                Button(action: { appState.refreshSources() }) {
                    Image(systemName: "arrow.clockwise")
                        .font(.system(size: 14))
                        .foregroundColor(.cyan)
                }
            }
            
            // Source list
            VStack(spacing: 6) {
                ForEach(appState.sourceDiscovery.sources) { source in
                    sourceRow(source)
                }
                
                if appState.sourceDiscovery.sources.isEmpty {
                    Text("Searching for sources...")
                        .font(.system(size: 13))
                        .foregroundColor(.white.opacity(0.5))
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 20)
                }
            }
            
            // Manual entry toggle
            Button(action: { showingManualEntry.toggle() }) {
                HStack {
                    Image(systemName: "plus.circle")
                    Text("Add Manual Source")
                }
                .font(.system(size: 13))
                .foregroundColor(.cyan)
            }
            
            if showingManualEntry {
                manualEntryView
            }
        }
    }
    
    private func sourceRow(_ source: SourceInfo) -> some View {
        Button(action: { appState.selectSource(source) }) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(source.displayName)
                        .font(.system(size: 14, weight: .medium))
                        .foregroundColor(.white)
                    
                    Text(source.address)
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundColor(.white.opacity(0.5))
                }
                
                Spacer()
                
                if appState.selectedSource?.id == source.id {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundColor(.green)
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
            .background(
                appState.selectedSource?.id == source.id
                    ? Color.white.opacity(0.1)
                    : Color.white.opacity(0.03),
                in: RoundedRectangle(cornerRadius: 8)
            )
        }
    }
    
    private var manualEntryView: some View {
        VStack(spacing: 10) {
            HStack(spacing: 10) {
                TextField("Host", text: $manualHost)
                    .textFieldStyle(.roundedBorder)
                    .autocapitalization(.none)
                    .keyboardType(.URL)
                
                TextField("Port", text: $manualPort)
                    .textFieldStyle(.roundedBorder)
                    .keyboardType(.numberPad)
                    .frame(width: 70)
            }
            
            Button(action: addManualSource) {
                Text("Add Source")
                    .font(.system(size: 14, weight: .medium))
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 10)
                    .background(Color.cyan.opacity(0.8), in: RoundedRectangle(cornerRadius: 8))
                    .foregroundColor(.white)
            }
        }
        .padding(.top, 8)
    }
    
    private func addManualSource() {
        guard !manualHost.isEmpty, let port = UInt16(manualPort) else { return }
        
        appState.sourceDiscovery.addManualSource(host: manualHost, port: port)
        manualHost = ""
        showingManualEntry = false
    }
    
    // MARK: - Zoom Reset
    
    private var zoomResetSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("View", systemImage: "viewfinder")
                .font(.system(size: 14, weight: .semibold))
                .foregroundColor(.white.opacity(0.8))
            
            HStack {
                Text("Zoom: \(String(format: "%.1fx", appState.zoomScale))")
                    .font(.system(size: 13, design: .monospaced))
                    .foregroundColor(.white.opacity(0.6))
                
                Spacer()
                
                Button(action: { appState.resetZoomPan() }) {
                    Text("Reset View")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundColor(.cyan)
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
            .background(Color.white.opacity(0.05), in: RoundedRectangle(cornerRadius: 8))
        }
    }
}

#Preview {
    ZStack {
        Color.black.ignoresSafeArea()
        OverlayMenuView()
            .environmentObject(AppState())
    }
}


