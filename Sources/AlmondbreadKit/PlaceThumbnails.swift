import Foundation

/// Thumbnails of the places catalogue (`places()` in the Rust core; kb: synthesis/places),
/// shipped as package resources: `Places/<id>.heic` (320x200) and `Places/<id>@2x.heic`
/// (640x400), rendered by `almondbread places --render-thumbs Sources/AlmondbreadKit/Resources/Places`.
/// Load them with `NSImage(contentsOf:)` / `UIImage(contentsOfFile:)`.
public enum PlaceThumbnails {
    public static func url(id: String, retina: Bool = true) -> URL? {
        Bundle.module.url(forResource: retina ? "\(id)@2x" : id, withExtension: "heic", subdirectory: "Places")
    }
}
