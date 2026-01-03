import SwiftUI

@main
struct ImmersiveReceiverApp: App {
    @StateObject private var appState = AppState()
    
    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(appState)
                .preferredColorScheme(.dark)
                .statusBarHidden(true)
                .modifier(HideSystemOverlaysModifier())
        }
    }
}

/// Modifier to hide system overlays on iOS 16+
struct HideSystemOverlaysModifier: ViewModifier {
    func body(content: Content) -> some View {
        if #available(iOS 16.0, *) {
            content.persistentSystemOverlays(.hidden)
        } else {
            content
        }
    }
}

