import SwiftUI
import ImmersiveReceiverCore
import MetalKit

struct ContentView: View {
    @EnvironmentObject var appState: AppState
    @State private var showingSourcePicker = false
    @State private var showingSettings = false
    @State private var hideControls = false
    
    var body: some View {
        ZStack {
            // Video view
            VideoView(renderer: appState.videoRenderer)
                .ignoresSafeArea()
            
            // Overlay UI (hidden when tapped)
            if !hideControls {
                VStack {
                    // Top bar
                    HStack {
                        // Connection status
                        ConnectionStatusView(state: appState.connectionState)
                        
                        Spacer()
                        
                        // Settings button
                        Button(action: { showingSettings = true }) {
                            Image(systemName: "gear")
                                .font(.title2)
                        }
                    }
                    .padding()
                    
                    Spacer()
                    
                    // Stats
                    if appState.connectionState.isConnected {
                        StatsView(resolution: appState.currentResolution, fps: appState.currentFps)
                            .padding()
                    }
                    
                    // Bottom bar
                    HStack {
                        // Receiver name
                        VStack(alignment: .leading) {
                            Text(appState.receiverName)
                                .font(.headline)
                            
                            if appState.controlServer.connectedClients > 0 {
                                Text("\(appState.controlServer.connectedClients) controller(s)")
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                            }
                        }
                        
                        Spacer()
                        
                        // Source picker button
                        Button(action: { showingSourcePicker = true }) {
                            Label("Sources", systemImage: "antenna.radiowaves.left.and.right")
                        }
                        .buttonStyle(.borderedProminent)
                        
                        // Disconnect button
                        if appState.connectionState.isConnected {
                            Button(action: { appState.disconnect() }) {
                                Image(systemName: "xmark.circle.fill")
                                    .font(.title2)
                            }
                            .foregroundColor(.red)
                        }
                    }
                    .padding()
                    .background(.ultraThinMaterial)
                }
            }
        }
        .onTapGesture {
            withAnimation(.easeInOut(duration: 0.2)) {
                hideControls.toggle()
            }
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
            .presentationDetents([.medium, .large])
        }
        .sheet(isPresented: $showingSettings) {
            SettingsView()
                .environmentObject(appState)
                .presentationDetents([.medium])
        }
        .onAppear {
            appState.sourceDiscovery.startBrowsing()
        }
        .statusBarHidden(hideControls)
    }
}

// MARK: - Video View

struct VideoView: View {
    let renderer: VideoRenderer?
    
    var body: some View {
        if let renderer = renderer {
            MetalVideoView(renderer: renderer)
        } else {
            Color.black
                .overlay {
                    VStack {
                        Image(systemName: "exclamationmark.triangle")
                            .font(.largeTitle)
                        Text("Metal not available")
                    }
                    .foregroundColor(.white)
                }
        }
    }
}

struct MetalVideoView: UIViewRepresentable {
    let renderer: VideoRenderer
    
    func makeUIView(context: Context) -> MTKView {
        let view = renderer.createView()
        view.contentMode = .scaleAspectFit
        return view
    }
    
    func updateUIView(_ uiView: MTKView, context: Context) {
        // View updates handled by renderer
    }
}

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
                .lineLimit(1)
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
        .foregroundColor(.white)
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(Capsule().fill(.black.opacity(0.5)))
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
                        ContentUnavailableView(
                            "No Sources Found",
                            systemImage: "antenna.radiowaves.left.and.right.slash",
                            description: Text("Make sure a sender is running on the same network")
                        )
                    }
                } else {
                    ForEach(sources) { source in
                        Button(action: { onSelect(source) }) {
                            HStack {
                                Image(systemName: "video.fill")
                                    .foregroundColor(.blue)
                                
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
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .primaryAction) {
                    Button(action: onRefresh) {
                        Image(systemName: "arrow.clockwise")
                    }
                }
            }
        }
    }
}

// MARK: - Settings View

struct SettingsView: View {
    @EnvironmentObject var appState: AppState
    @State private var editedName: String = ""
    @Environment(\.dismiss) private var dismiss
    
    var body: some View {
        NavigationStack {
            Form {
                Section("Receiver Identity") {
                    TextField("Display Name", text: $editedName)
                        .onAppear { editedName = appState.receiverName }
                    
                    Text("This name will be visible to controllers on the network")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                
                Section("Network Status") {
                    LabeledContent("Control Port") {
                        Text("\(appState.controlServer.port)")
                    }
                    
                    LabeledContent("Server Status") {
                        HStack {
                            Circle()
                                .fill(appState.controlServer.isRunning ? .green : .red)
                                .frame(width: 8, height: 8)
                            Text(appState.controlServer.isRunning ? "Running" : "Stopped")
                        }
                    }
                    
                    LabeledContent("Controllers") {
                        Text("\(appState.controlServer.connectedClients)")
                    }
                    
                    LabeledContent("Broadcasting") {
                        HStack {
                            Circle()
                                .fill(appState.receiverBroadcast.isAdvertising ? .green : .red)
                                .frame(width: 8, height: 8)
                            Text(appState.receiverBroadcast.isAdvertising ? "Active" : "Inactive")
                        }
                    }
                }
            }
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") {
                        if editedName != appState.receiverName && !editedName.isEmpty {
                            appState.receiverName = editedName
                        }
                        dismiss()
                    }
                }
            }
        }
    }
}

#Preview {
    ContentView()
        .environmentObject(AppState())
}







