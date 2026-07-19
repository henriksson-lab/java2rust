package examples.stdlib;

public class NestedStaticBuilderCall {
    public Builder make() {
        Builder builder = Coordinates.GenomicsDBColumn.newBuilder();
        return builder;
    }

    static class Coordinates {
        static class GenomicsDBColumn {
            static Builder newBuilder() {
                return new Builder();
            }
        }
    }

    static class Builder {
    }
}
