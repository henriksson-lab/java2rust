import java.io.IOException;
import java.io.UncheckedIOException;
import java.util.Arrays;
import org.apache.commons.csv.CSVFormat;
import org.apache.commons.csv.CSVParser;
import org.apache.commons.csv.CSVPrinter;
import org.apache.commons.csv.CSVRecord;
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

    private static CSVRecord one(String input, CSVFormat format) throws IOException {
        try (CSVParser parser = CSVParser.parse(input, format)) {
            return parser.iterator().next();
        }
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
    }
}
