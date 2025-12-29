import SwiftUI
import ImmersiveReceiverCore
import MetalKit

struct ContentView: View {
    @EnvironmentObject var appState: AppState
    @State private var showingSourcePicker = false
    @State private var isFullscreen = false
    
    var body: some View {
        ZStack {
            // Video view
            VideoView(renderer: appState.videoRenderer)
                .ignoresSafeArea()
            
            // Overlay UI (hidden in fullscreen)
            if !isFullscreen {
                VStack {
                    // Top bar
                    HStack {
                        // Connection status
                        ConnectionStatusView(state: appState.connectionState)
                        
                        Spacer()
                        
                        // Stats
                        if appState.connectionState.isConnected {
                            StatsView(resolution: appState.currentResolution, fps: appState.currentFps)
                        }
                        
                        Spacer()
                        
                        // Source picker button
                        Button(action: { showingSourcePicker.toggle() }) {
                            Label("Sources", systemImage: "antenna.radiowaves.left.and.right")
                        }
                        .buttonStyle(.bordered)
                    }
                    .padding()
                    .background(.ultraThinMaterial)
                    
                    Spacer()
                    
                    // Bottom bar - receiver info
                    HStack {
                        Image(systemName: "display")
                        Text(appState.receiverName)
                            .font(.headline)
                        
                        Spacer()
                        
                        if appState.controlServer.isRunning {
                            Label("\(appState.controlServer.connectedClients) controllers", systemImage: "network")
                                .foregroundColor(.secondary)
                        }
                    }
                    .padding()
                    .background(.ultraThinMaterial)
                }
            }
        }
        .onTapGesture(count: 2) {
            toggleFullscreen()
        }
        .sheet(isPresented: $showingSourcePicker) {
            SourcePickerView(
                sources: appState.sourceDiscovery.sources,
                isSearching: appState.sourceDiscovery.isSearching,
                onSelect: { source in
                    appState.connectToSource(source)
                    showingSourcePicker = false
                },
                onRefresh: {
                    appState.sourceDiscovery.refresh()
                }
            )
        }
        .onAppear {
            appState.sourceDiscovery.startBrowsing()
        }
    }
    
    private func toggleFullscreen() {
        withAnimation(.easeInOut(duration: 0.3)) {
            isFullscreen.toggle()
        }
        
        #if os(macOS)
        if let window = NSApplication.shared.windows.first {
            window.toggleFullScreen(nil)
        }
        #endif
    }
}

// MARK: - Video View

struct VideoView: View {
    let renderer: VideoRenderer?
    
    var body: some View {
        if let renderer = renderer {
            MetalVideoView(renderer: renderer)
        } else {
            // Show a placeholder when no video or Metal not available
            ZStack {
                Color.black
                
                VStack(spacing: 20) {
                    Image(systemName: "tv")
                        .font(.system(size: 60))
                        .foregroundColor(.gray)
                    
                    Text("Waiting for video...")
                        .font(.title2)
                        .foregroundColor(.gray)
                    
                    Text("Select a source to start receiving")
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                }
            }
        }
    }
}

#if os(macOS)
struct MetalVideoView: NSViewRepresentable {
    let renderer: VideoRenderer
    
    func makeNSView(context: Context) -> MTKView {
        renderer.createView()
    }
    
    func updateNSView(_ nsView: MTKView, context: Context) {
        // View updates handled by renderer
    }
}
#else
struct MetalVideoView: UIViewRepresentable {
    let renderer: VideoRenderer
    
    func makeUIView(context: Context) -> MTKView {
        renderer.createView()
    }
    
    func updateUIView(_ uiView: MTKView, context: Context) {
        // View updates handled by renderer
    }
}
#endif

// MARK: - Connection Status

struct ConnectionStatusView: View {
    let state: ConnectionState
    
    var body: some View {
        HStack(spacing: 8) {
            Circle()
                .fill(statusColor)
                .frame(width: 10, height: 10)
            
            Text(state.displayText)
                .font(.subheadline)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(Capsule().fill(.ultraThinMaterial))
    }
    
    private var statusColor: Color {
        switch state {
        case .disconnected:
            return .gray
        case .connecting:
            return .orange
        case .connected:
            return .green
        case .error:
            return .red
        }
    }
}

// MARK: - Stats View

struct StatsView: View {
    let resolution: String
    let fps: Int
    
    var body: some View {
        HStack(spacing: 16) {
            Label(resolution, systemImage: "rectangle.on.rectangle")
            Label("\(fps) fps", systemImage: "speedometer")
        }
        .font(.caption)
        .foregroundColor(.secondary)
    }
}

// MARK: - Source Picker

struct SourcePickerView: View {
    let sources: [SourceInfo]
    let isSearching: Bool
    let onSelect: (SourceInfo) -> Void
    let onRefresh: () -> Void
    
    @Environment(\.dismiss) private var dismiss
    
    var body: some View {
        NavigationStack {
            List {
                if sources.isEmpty {
                    if isSearching {
                        HStack {
                            ProgressView()
                            Text("Searching for sources...")
                                .foregroundColor(.secondary)
                        }
                    } else {
                        Text("No sources found")
                            .foregroundColor(.secondary)
                    }
                } else {
                    ForEach(sources) { source in
                        Button(action: { onSelect(source) }) {
                            HStack {
                                VStack(alignment: .leading) {
                                    Text(source.displayName)
                                        .font(.headline)
                                    Text(source.address)
                                        .font(.caption)
                                        .foregroundColor(.secondary)
                                }
                                
                                Spacer()
                                
                                Image(systemName: "chevron.right")
                                    .foregroundColor(.secondary)
                            }
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            .navigationTitle("Select Source")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .primaryAction) {
                    Button(action: onRefresh) {
                        Label("Refresh", systemImage: "arrow.clockwise")
                    }
                }
            }
        }
        .frame(minWidth: 400, minHeight: 300)
    }
}

// MARK: - Settings View

struct SettingsView: View {
    @EnvironmentObject var appState: AppState
    @State private var editedName: String = ""
    
    var body: some View {
        Form {
            Section("Receiver Identity") {
                TextField("Display Name", text: $editedName)
                    .onAppear { editedName = appState.receiverName }
                    .onSubmit { appState.receiverName = editedName }
                
                Text("This name will be visible to controllers on the network")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            
            Section("Network") {
                LabeledContent("Control Port") {
                    Text("\(appState.controlServer.port)")
                }
                
                LabeledContent("Status") {
                    HStack {
                        Circle()
                            .fill(appState.controlServer.isRunning ? .green : .red)
                            .frame(width: 8, height: 8)
                        Text(appState.controlServer.isRunning ? "Running" : "Stopped")
                    }
                }
                
                LabeledContent("Connected Controllers") {
                    Text("\(appState.controlServer.connectedClients)")
                }
            }
            
            Section("Broadcast") {
                LabeledContent("Advertising") {
                    HStack {
                        Circle()
                            .fill(appState.receiverBroadcast.isAdvertising ? .green : .red)
                            .frame(width: 8, height: 8)
                        Text(appState.receiverBroadcast.isAdvertising ? "Active" : "Inactive")
                    }
                }
            }
        }
        .padding()
        .frame(width: 450, height: 300)
    }
}

#Preview {
    ContentView()
        .environmentObject(AppState())
}


