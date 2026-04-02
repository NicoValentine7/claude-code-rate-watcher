import CCRWCore
import Foundation
import Testing

struct SessionParserTests {
    @Test
    func parseSessionFileDeduplicatesAssistantMessages() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let fileURL = directory.appendingPathComponent("session.jsonl")
        let contents = """
        {"type":"assistant","timestamp":"2026-04-03T01:00:00Z","message":{"id":"msg-1","role":"assistant","usage":{"input_tokens":100,"output_tokens":20,"cache_creation_input_tokens":5,"cache_read_input_tokens":1}}}
        {"type":"assistant","timestamp":"2026-04-03T01:01:00Z","message":{"id":"msg-1","role":"assistant","usage":{"input_tokens":100,"output_tokens":30,"cache_creation_input_tokens":5,"cache_read_input_tokens":1}}}
        {"type":"assistant","timestamp":"2026-04-03T01:02:00Z","message":{"id":"msg-2","role":"assistant","usage":{"input_tokens":40,"output_tokens":5,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}}
        {"type":"user","timestamp":"2026-04-03T01:03:00Z","message":{"id":"ignored","role":"user","usage":{"input_tokens":999,"output_tokens":999,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}}
        """
        try contents.write(to: fileURL, atomically: true, encoding: .utf8)

        let records = SessionParser.parseSessionFile(at: fileURL)

        #expect(records.count == 2)
        let first = try #require(records.first(where: { $0.messageID == "msg-1" }))
        #expect(first.usage.outputTokens == 30)
        let second = try #require(records.first(where: { $0.messageID == "msg-2" }))
        #expect(second.usage.inputTokens == 40)
    }
}
