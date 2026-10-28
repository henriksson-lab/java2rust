import java.io.IOException;
import java.io.UncheckedIOException;
import java.util.Arrays;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;
import java.util.stream.Stream;
import org.apache.commons.csv.CSVFormat;
import org.apache.commons.csv.CSVParser;
import org.apache.commons.csv.CSVPrinter;
import org.apache.commons.csv.CSVRecord;
import org.apache.commons.csv.CommonsCsvReaderOracle;
import org.apache.commons.csv.QuoteMode;

/** Emits a stable behavioral transcript from untouched Apache Commons CSV. */
public final class CommonsCsvOracle {
    private static CSVFormat format(String delimiter, String nullString) {
        return CSVFormat.Builder.create()
                .setDelimiter(delimiter)
                .setQuote('"')
                .setQuoteMode(QuoteMode.MINIMAL)
                .setRecordSeparator("\r\n")
                .setNullString(nullString)
                .build();
    }

    private static String visible(String value) {
        return value == null ? "<null>" : value.replace("\\", "\\\\")
                .replace("\r", "\\r").replace("\n", "\\n").replace("|", "\\|");
    }

    private static String values(CSVRecord record) {
        return Arrays.stream(record.values()).map(CommonsCsvOracle::visible)
                .reduce((left, right) -> left + "|" + right).orElse("");
    }

    private static String codeUnits(String value) {
        if (value == null) return "<null>";
        StringBuilder result = new StringBuilder();
        for (int i = 0; i < value.length(); i++) {
            if (i > 0) result.append('.');
            result.append(String.format("%04x", (int) value.charAt(i)));
        }
        return result.toString();
    }

    private static String unitValues(CSVRecord record) {
        return Arrays.stream(record.values()).map(CommonsCsvOracle::codeUnits)
                .reduce((left, right) -> left + "|" + right).orElse("");
    }

    private static CSVRecord one(String input, CSVFormat format) throws IOException {
        try (CSVParser parser = CSVParser.parse(input, format)) {
            return parser.iterator().next();
        }
    }

    private static Stream<CSVRecord> escapedStream() throws IOException {
        CSVParser parser = CSVParser.parse("escaped,1\r\nowner,2\r\n", format(",", null));
        return parser.stream();
    }

    public static void main(String[] args) throws Exception {
        StringBuilder out = new StringBuilder();
        try (CSVPrinter printer = new CSVPrinter(out, format(",", null))) {
            printer.printRecord("a", "b,c");
        }
        System.out.println("print=" + visible(out.toString()));
        System.out.println("roundtrip=" + values(one(out.toString(), format(",", null))));
        System.out.println("quoted=" + values(one("\"a\"\"b\",\"line1\r\nline2\",,\r\n", format(",", null))));
        System.out.println("delimiter=" + values(one("left;\"x;y\"\r\n", format(";", null))));
        System.out.println("nulls=" + values(one("NULL,\"NULL\",,\r\n", format(",", "NULL"))));

        CSVFormat headers = CSVFormat.Builder.create()
                .setDelimiter(',').setQuote('"').setQuoteMode(QuoteMode.MINIMAL)
                .setRecordSeparator("\r\n").setCommentMarker('#').setHeader().build();
        try (CSVParser parser = CSVParser.parse("# generated\r\nname,age\r\nAda,42\r\n", headers)) {
            CSVRecord record = parser.iterator().next();
            System.out.println("headers=" + String.join("|", parser.getHeaderNames()));
            System.out.println("comment=" + visible(parser.getHeaderComment()));
            System.out.println("named=" + visible(record.get("name")) + "|" + visible(record.get("age")));
        }

        try {
            one("\"unclosed", format(",", null));
            System.out.println("malformed=accepted");
        } catch (IOException | UncheckedIOException expected) {
            System.out.println("malformed=IOException");
        }

        System.out.println("utf16=" + unitValues(one("\ud800,\udfff,\ud83d\ude80\r\n", format(",", null))));
        StringBuilder rawOut = new StringBuilder();
        try (CSVPrinter printer = new CSVPrinter(rawOut, format(",", null))) {
            printer.printRecord("\ud800,\udfff");
        }
        System.out.println("utf16-print=" + codeUnits(rawOut.toString()));
        CSVFormat whitespace = CSVFormat.Builder.create().setIgnoreSurroundingSpaces(true).build();
        String spaced = "\u001ca\u001c,\u0085b\u0085,\u00a0c\u00a0,\u2003d\u2003,\u2007e\u2007,\u202ff\u202f\r\n";
        System.out.println("whitespace=" + unitValues(one(spaced, whitespace)));

        try (CSVParser parser = CSVParser.parse("a,1\r\nb,2\r\nc,3\r\n", format(",", null))) {
            Stream<CSVRecord> stream = parser.stream();
            long before = parser.getRecordNumber();
            parser.iterator().next();
            System.out.println("stream-alias=" + before + "|" + stream.count() + "|" + parser.getRecordNumber());
            try { stream.count(); System.out.println("stream-reuse=accepted"); }
            catch (IllegalStateException expected) { System.out.println("stream-reuse=IllegalStateException"); }
        }
        try (Stream<CSVRecord> stream = escapedStream()) {
            System.out.println("stream-escape=" + stream.count());
        }
        try (CSVParser parser = CSVParser.parse("a\r\nb\r\n", format(",", null))) {
            Stream<CSVRecord> stream = parser.stream();
            stream.close();
            System.out.println("stream-close=" + parser.isClosed() + "|" + parser.getRecords().size());
            try { stream.count(); System.out.println("stream-closed-use=accepted"); }
            catch (IllegalStateException expected) { System.out.println("stream-closed-use=IllegalStateException"); }
        }
        try (CSVParser parser = CSVParser.parse("a\r\nb\r\n", format(",", null))) {
            Stream<CSVRecord> stream = parser.stream();
            parser.close();
            System.out.println("parser-close=" + stream.count());
        }
        try (CSVParser parser = CSVParser.parse("a\r\nb\r\n", format(",", null))) {
            boolean matched = parser.stream().anyMatch(record -> {
                try { parser.close(); } catch (IOException error) { throw new UncheckedIOException(error); }
                return true;
            });
            System.out.println("stream-reentry=" + matched + "|" + parser.getRecordNumber() + "|" + parser.isClosed());
        }
        try (CSVParser parser = CSVParser.parse("\"unclosed", format(",", null))) {
            Stream<CSVRecord> stream = parser.stream();
            long before = parser.getRecordNumber();
            try { stream.count(); System.out.println("stream-malformed=accepted"); }
            catch (UncheckedIOException expected) { System.out.println("stream-malformed=" + before + "|UncheckedIOException"); }
        }

        try (CSVParser parser = CSVParser.parse("left,right\r\n", format(",", null))) {
            CSVRecord record = parser.iterator().next();
            String[] array = record.values();
            List<String> snapshot = record.toList();
            Stream<String> stream = record.stream();
            array[0] = "changed";
            array[1] = null;
            System.out.println("array-alias=" + (array == record.values()) + "|" + visible(record.get(0)));
            System.out.println("record-stream=" + stream.map(CommonsCsvOracle::visible).collect(Collectors.joining("|")));
            System.out.println("list-copy=" + snapshot.stream().map(CommonsCsvOracle::visible).collect(Collectors.joining("|")));
            array[0] = "one";
            array[1] = "two";
            List<String> seen = new ArrayList<>();
            record.stream().forEach(value -> { seen.add(value); if (seen.size() == 1) array[1] = "after"; });
            System.out.println("array-reentry=" + String.join("|", seen));
            Map<String, String> noHeader = new LinkedHashMap<>();
            noHeader.put("sentinel", "keep");
            System.out.println("map-no-header=" + (record.putIn(noHeader) == noHeader) + "|" + noHeader.size());
        }
        try (CSVParser parser = CSVParser.parse("a,b\r\nx,y\r\n", headers)) {
            CSVRecord record = parser.iterator().next();
            record.values()[1] = null;
            Map<String, String> map = new LinkedHashMap<>();
            map.put("sentinel", "keep");
            map.put("a", "old");
            map.put("b", "old");
            Map<String, String> returned = record.putIn(map);
            returned.put("later", "added");
            System.out.println("map-identity=" + (returned == map) + "|" + map.size() + "|"
                    + map.containsKey("b") + "|" + visible(map.get("a")) + "|" + visible(map.get("b"))
                    + "|" + visible(map.get("sentinel")) + "|" + visible(map.get("later")));
            Map<String, String> copy = record.toMap();
            copy.put("a", "copy");
            System.out.println("map-copy=" + visible(record.get("a")) + "|" + visible(copy.get("a")) + "|"
                    + copy.containsKey("b") + "|" + visible(copy.get("b")));
        }
        StringBuilder nestedOut = new StringBuilder();
        try (CSVPrinter printer = new CSVPrinter(nestedOut, format(",", null))) {
            printer.printRecords(new Object[] {new String[] {"a", "b"}, Arrays.asList("c", null)});
        }
        System.out.println("nested-print=" + visible(nestedOut.toString()));
        StringBuilder streamOut = new StringBuilder();
        try (CSVPrinter printer = new CSVPrinter(streamOut, format(",", null))) {
            printer.printRecord(Stream.of("s", "t"));
        }
        System.out.println("stream-print=" + visible(streamOut.toString()));
        System.out.println("reader-lookahead=" + CommonsCsvReaderOracle.closedLookAhead());
        System.out.println("delimiter-multi=" + values(one("left||\"x||y\"\r\n", format("||", null))));
        CSVFormat escapedFormat = CSVFormat.Builder.create().setQuote('"').setEscape('\\').build();
        System.out.println("escaped=" + values(one("\"a\\\"b\",c\\,d\r\n", escapedFormat)));
    }
}
