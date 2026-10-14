package htsjdk.variant.vcf;

import java.util.Collections;
import java.util.List;

/** Narrow dependency boundary required by VCFHeaderLine.isHeaderLine. */
public final class VCFHeader {
    public static final String HEADER_INDICATOR = "#";
    public static final String METADATA_INDICATOR = "##";

    public boolean hasInfoLine(final String key) { return false; }
    public boolean hasFormatLine(final String key) { return false; }
    public boolean hasFilterLine(final String key) { return false; }
    public List<String> getGenotypeSamples() { return Collections.emptyList(); }
    public VCFInfoHeaderLine getInfoHeaderLine(final String key) { return null; }
}
