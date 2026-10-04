// 屏幕阅读用的 OCR 小工具：调用系统自带的 Vision，完全在本机识别，不联网。
// 用法：subtext-ocr <图片路径> [--fast]
// 输出一行 JSON：{"width": 像素宽, "height": 像素高, "lines": [{"text", "x", "y", "w", "h", "confidence"}]}
// 坐标是 0 到 1 的比例，原点在图片左上角。

import AppKit
import Foundation
import Vision

let arguments = Array(CommandLine.arguments.dropFirst())
guard let path = arguments.first(where: { !$0.hasPrefix("--") }) else {
    FileHandle.standardError.write(Data("usage: subtext-ocr <image> [--fast]\n".utf8))
    exit(2)
}
guard let image = NSImage(contentsOfFile: path),
      let cgImage = image.cgImage(forProposedRect: nil, context: nil, hints: nil)
else {
    FileHandle.standardError.write(Data("cannot read image: \(path)\n".utf8))
    exit(1)
}

let request = VNRecognizeTextRequest()
// 屏幕文字清晰，.fast 对英文够用且快几倍；中文要 .accurate 才不丢字
request.recognitionLevel = arguments.contains("--fast") ? .fast : .accurate
request.usesLanguageCorrection = true
request.recognitionLanguages = ["zh-Hans", "en-US"]

do {
    try VNImageRequestHandler(cgImage: cgImage, options: [:]).perform([request])
} catch {
    FileHandle.standardError.write(Data("vision failed: \(error)\n".utf8))
    exit(1)
}

var lines: [[String: Any]] = []
for observation in request.results ?? [] {
    guard let candidate = observation.topCandidates(1).first else { continue }
    let box = observation.boundingBox  // 归一化，原点在左下角
    lines.append([
        "text": candidate.string,
        "x": box.minX,
        "y": 1.0 - box.maxY,
        "w": box.width,
        "h": box.height,
        "confidence": candidate.confidence,
    ])
}
let result: [String: Any] = ["width": cgImage.width, "height": cgImage.height, "lines": lines]
let data = try JSONSerialization.data(withJSONObject: result, options: [.withoutEscapingSlashes])
FileHandle.standardOutput.write(data)
FileHandle.standardOutput.write(Data("\n".utf8))
