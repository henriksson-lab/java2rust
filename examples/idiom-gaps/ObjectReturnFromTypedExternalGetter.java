import htsjdk.tribble.FeatureReader;

class ObjectReturnFromTypedExternalGetter {
    Object header(FeatureReader reader) {
        return reader.getHeader();
    }
}
