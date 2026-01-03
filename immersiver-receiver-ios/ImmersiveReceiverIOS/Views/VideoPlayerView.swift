import SwiftUI
import MetalKit

/// SwiftUI wrapper for the Metal-based video renderer
struct VideoPlayerView: UIViewRepresentable {
    @EnvironmentObject var appState: AppState
    
    func makeUIView(context: Context) -> MTKView {
        guard let renderer = appState.videoRenderer else {
            // Return a placeholder view if Metal is not available
            let view = MTKView()
            view.backgroundColor = .black
            return view
        }
        
        let view = renderer.createView()
        view.backgroundColor = .black
        return view
    }
    
    func updateUIView(_ uiView: MTKView, context: Context) {
        // Apply zoom and pan transforms
        let scale = appState.zoomScale
        let offset = appState.panOffset
        
        uiView.transform = CGAffineTransform(scaleX: scale, y: scale)
            .translatedBy(x: offset.width, y: offset.height)
    }
}

#Preview {
    VideoPlayerView()
        .environmentObject(AppState())
}

