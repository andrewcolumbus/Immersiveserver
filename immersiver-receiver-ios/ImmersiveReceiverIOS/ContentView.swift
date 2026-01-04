import SwiftUI
import MetalKit

struct ContentView: View {
    @EnvironmentObject var appState: AppState
    
    var body: some View {
        ZStack {
            // Video player view with zoom/pan
            VideoPlayerView()
                .ignoresSafeArea()
            
            // Tap gesture layer
            Color.clear
                .contentShape(Rectangle())
                .onTapGesture {
                    appState.toggleMenu()
                }
            
            // Overlay menu
            if appState.isMenuVisible {
                OverlayMenuView()
                    .transition(.opacity.combined(with: .scale(scale: 0.95)))
            }
            
            // Connection status indicator (when menu hidden)
            if !appState.isMenuVisible {
                VStack {
                    HStack {
                        Spacer()
                        ConnectionIndicator()
                            .padding()
                    }
                    Spacer()
                }
            }
        }
        .background(Color.black)
        .gesture(zoomGesture)
        .gesture(panGesture)
    }
    
    // MARK: - Gestures
    
    private var zoomGesture: some Gesture {
        MagnificationGesture()
            .onChanged { value in
                let newScale = value * appState.zoomScale
                appState.zoomScale = min(max(newScale, 0.5), 5.0)
            }
    }
    
    private var panGesture: some Gesture {
        DragGesture()
            .onChanged { value in
                if appState.zoomScale > 1.0 {
                    appState.panOffset = CGSize(
                        width: appState.panOffset.width + value.translation.width / appState.zoomScale,
                        height: appState.panOffset.height + value.translation.height / appState.zoomScale
                    )
                }
            }
    }
}

// MARK: - Connection Indicator

struct ConnectionIndicator: View {
    @EnvironmentObject var appState: AppState
    
    var body: some View {
        HStack(spacing: 6) {
            Circle()
                .fill(indicatorColor)
                .frame(width: 8, height: 8)
            
            if appState.mode == .camera {
                Text("LIVE")
                    .font(.system(size: 10, weight: .bold, design: .monospaced))
                    .foregroundColor(.white)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(.ultraThinMaterial, in: Capsule())
    }
    
    private var indicatorColor: Color {
        switch appState.connectionState {
        case .connected:
            return .green
        case .connecting:
            return .yellow
        case .error:
            return .red
        case .disconnected:
            return .gray
        }
    }
}

// MARK: - Preview

#Preview {
    ContentView()
        .environmentObject(AppState())
}


