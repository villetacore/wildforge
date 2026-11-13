import Foundation
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers

let args = CommandLine.arguments
let source = CGImageSourceCreateWithURL(URL(fileURLWithPath: args[1]) as CFURL, nil)!
let image = CGImageSourceCreateImageAtIndex(source, 0, nil)!
let columns = Int(args[2])!
let names = args[3].split(separator: ",")
for (index, name) in names.enumerated() {
    let x = index % columns * image.width / columns
    let y = index / columns * image.height / columns
    let width = (index % columns + 1) * image.width / columns - x
    let height = (index / columns + 1) * image.height / columns - y
    let crop = image.cropping(to: CGRect(x: x + 2, y: y + 2, width: width - 4, height: height - 4))!
    let context = CGContext(data: nil, width: 256, height: 256, bitsPerComponent: 8,
        bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(),
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    context.interpolationQuality = .high
    context.draw(crop, in: CGRect(x: 0, y: 0, width: 256, height: 256))
    let url = URL(fileURLWithPath: "content/textures/\(name).png")
    let output = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)!
    CGImageDestinationAddImage(output, context.makeImage()!, nil)
    CGImageDestinationFinalize(output)
}
