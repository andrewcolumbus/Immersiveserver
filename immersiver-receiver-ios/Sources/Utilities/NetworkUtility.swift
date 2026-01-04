import Foundation
import Network

/// Utility functions for network-related operations
public enum NetworkUtility {
    
    /// Get the device's WiFi IP address
    /// Returns nil if not connected to WiFi or unable to determine address
    public static func getWiFiIPAddress() -> String? {
        var address: String?
        
        // Get list of all interfaces on the local machine
        var ifaddr: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&ifaddr) == 0 else { return nil }
        defer { freeifaddrs(ifaddr) }
        
        var ptr = ifaddr
        while ptr != nil {
            defer { ptr = ptr?.pointee.ifa_next }
            
            guard let interface = ptr?.pointee else { continue }
            
            // Check for IPv4 or IPv6 interface
            let addrFamily = interface.ifa_addr.pointee.sa_family
            if addrFamily == UInt8(AF_INET) || addrFamily == UInt8(AF_INET6) {
                
                // Check interface name
                let name = String(cString: interface.ifa_name)
                
                // Look for en0 (WiFi on iOS) or en1 (Ethernet on some devices)
                if name == "en0" || name == "en1" {
                    
                    // Convert interface address to a human readable string
                    var hostname = [CChar](repeating: 0, count: Int(NI_MAXHOST))
                    
                    let saLen: socklen_t
                    if addrFamily == UInt8(AF_INET) {
                        saLen = socklen_t(MemoryLayout<sockaddr_in>.size)
                    } else {
                        saLen = socklen_t(MemoryLayout<sockaddr_in6>.size)
                    }
                    
                    if getnameinfo(
                        interface.ifa_addr,
                        saLen,
                        &hostname,
                        socklen_t(hostname.count),
                        nil,
                        0,
                        NI_NUMERICHOST
                    ) == 0 {
                        let addr = String(cString: hostname)
                        
                        // Prefer IPv4 addresses
                        if addrFamily == UInt8(AF_INET) {
                            address = addr
                            break
                        } else if address == nil {
                            // Use IPv6 as fallback if no IPv4 found
                            address = addr
                        }
                    }
                }
            }
        }
        
        return address
    }
    
    /// Get all available network interfaces with their addresses
    public static func getAllInterfaces() -> [(name: String, address: String, isIPv4: Bool)] {
        var interfaces: [(name: String, address: String, isIPv4: Bool)] = []
        
        var ifaddr: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&ifaddr) == 0 else { return interfaces }
        defer { freeifaddrs(ifaddr) }
        
        var ptr = ifaddr
        while ptr != nil {
            defer { ptr = ptr?.pointee.ifa_next }
            
            guard let interface = ptr?.pointee else { continue }
            
            let addrFamily = interface.ifa_addr.pointee.sa_family
            if addrFamily == UInt8(AF_INET) || addrFamily == UInt8(AF_INET6) {
                
                let name = String(cString: interface.ifa_name)
                var hostname = [CChar](repeating: 0, count: Int(NI_MAXHOST))
                
                let saLen: socklen_t
                if addrFamily == UInt8(AF_INET) {
                    saLen = socklen_t(MemoryLayout<sockaddr_in>.size)
                } else {
                    saLen = socklen_t(MemoryLayout<sockaddr_in6>.size)
                }
                
                if getnameinfo(
                    interface.ifa_addr,
                    saLen,
                    &hostname,
                    socklen_t(hostname.count),
                    nil,
                    0,
                    NI_NUMERICHOST
                ) == 0 {
                    let addr = String(cString: hostname)
                    interfaces.append((name: name, address: addr, isIPv4: addrFamily == UInt8(AF_INET)))
                }
            }
        }
        
        return interfaces
    }
    
    /// Check if the device is connected to a network
    public static func isConnectedToNetwork() -> Bool {
        return getWiFiIPAddress() != nil
    }
}


