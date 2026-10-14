package htsjdk.variant.vcf;

import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

public final class HtsjdkVcfOracle {
    private static void emitMap(final String label, final Map<String, String> map) {
        final StringBuilder result = new StringBuilder();
        for (final String key : map.keySet()) {
            if (result.length() != 0) result.append('|');
            result.append(key).append('=').append(map.get(key));
        }
        System.out.println(label + "=" + result);
    }

    private static void emitFailure(final String label, final Runnable operation) {
        try {
            operation.run();
            System.out.println(label + "=OK");
        } catch (final RuntimeException failure) {
            System.out.println(label + "=" + failure.getClass().getSimpleName());
        }
    }

    public static void main(final String[] args) {
        final VCFPercentEncodedTextTransformer percent = new VCFPercentEncodedTextTransformer();
        for (final String input : Arrays.asList("plain", "%3D%41", "a%20b", "%GG", "%", "%25", "%09")) {
            System.out.println("percent:" + input + "=" + percent.decodeText(input));
        }
        System.out.println("percent-list=" + String.join("|", percent.decodeText(Arrays.asList("%41", "x%2Cy"))));

        final VCFPassThruTextTransformer pass = new VCFPassThruTextTransformer();
        System.out.println("pass=" + pass.decodeText("%41"));

        for (final VCFHeaderVersion version : VCFHeaderVersion.values()) {
            System.out.println("version=" + version.name() + ":" + version.ordinal() + ":"
                    + version.getVersionString() + ":" + version.getFormatString());
        }
        System.out.println("version-parse=" + VCFHeaderVersion.getHeaderVersion("##fileformat=VCFv4.3"));
        System.out.println("version-recent=" + VCFHeaderVersion.VCF4_3.isAtLeastAsRecentAs(VCFHeaderVersion.VCF4_1));
        emitFailure("version-invalid", () -> VCFHeaderVersion.getHeaderVersion("##fileformat=VCFv9.9"));

        for (final VCFHeaderLineCount value : VCFHeaderLineCount.values()) {
            System.out.println("count=" + value.name() + ":" + value.ordinal());
        }
        for (final VCFHeaderLineType value : VCFHeaderLineType.values()) {
            System.out.println("type=" + value.name() + ":" + value.ordinal());
        }

        final List<String> expected = Arrays.asList("ID", "Description");
        final Map<String, String> parsed = VCFHeaderLineTranslator.parseLine(
                VCFHeaderVersion.VCF4_3, "<ID=q10,Description=\"quality, below ten\">", expected);
        emitMap("parse4", parsed);
        emitMap("parse3", VCFHeaderLineTranslator.parseLine(
                VCFHeaderVersion.VCF3_3, "q10,\"quality, below ten\"", expected));
        emitFailure("parse-unclosed", () -> VCFHeaderLineTranslator.parseLine(
                VCFHeaderVersion.VCF4_3, "<ID=q10,Description=\"open>", expected));
        emitFailure("parse-order", () -> VCFHeaderLineTranslator.parseLine(
                VCFHeaderVersion.VCF4_3, "<Description=x,ID=q10>", expected));

        final VCFSimpleHeaderLine direct = new VCFSimpleHeaderLine("FILTER", "q10", "quality, below ten");
        System.out.println("simple=" + direct);
        System.out.println("metadata=" + direct.getID() + ":" + direct.getGenericFields().get("Description"));

        final LinkedHashMap<String, String> fields = new LinkedHashMap<>();
        fields.put("ID", "q20");
        fields.put("Description", "has a \"quote\"");
        final VCFSimpleHeaderLine fromMap = new VCFSimpleHeaderLine("FILTER", fields);
        System.out.println("simple-map=" + fromMap);

        final String encoded = direct.toString();
        final VCFSimpleHeaderLine roundTrip = new VCFSimpleHeaderLine(
                encoded.substring(encoded.indexOf('=') + 1), VCFHeaderVersion.VCF4_3, "FILTER", expected);
        System.out.println("roundtrip=" + roundTrip.getID() + ":"
                + roundTrip.getGenericFields().get("Description") + ":" + roundTrip);
        emitFailure("simple-invalid", () -> new VCFSimpleHeaderLine("FILTER", "bad<id", "x"));

        final double[] recordDoubles = {0.0, 0.005, 0.01, 0.9996, 1.0, 12.345, -0.25};
        for (int i = 0; i < recordDoubles.length; i++) {
            System.out.println("record-double:" + i + "=" + VCFEncoder.formatVCFDouble(recordDoubles[i]));
        }
        System.out.println("record-field:null=" + VCFEncoder.formatVCFField(null));
        System.out.println("record-field:double=" + VCFEncoder.formatVCFField(0.125));
        System.out.println("record-field:true=" + VCFEncoder.formatVCFField(true));
        System.out.println("record-field:false=" + String.valueOf(VCFEncoder.formatVCFField(false)));
        System.out.println("record-field:string=" + VCFEncoder.formatVCFField("A,C"));
        System.out.println("record-count=" + VCFEncoder.countOccurrences(',', "A,C,,G"));
        for (final String value : Arrays.asList(".", ".,.", ".,", "A", "")) {
            System.out.println("record-missing:" + value + "=" + VCFEncoder.isMissingValue(value));
        }
    }
}
