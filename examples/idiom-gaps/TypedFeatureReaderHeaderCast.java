import htsjdk.tribble.FeatureReader;
import htsjdk.variant.vcf.VCFHeader;

class TypedFeatureReaderHeaderCast {
    VCFHeader header(FeatureReader reader) {
        return (VCFHeader) reader.getHeader();
    }
}
