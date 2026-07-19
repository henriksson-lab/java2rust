package examples.stdlib;

import java.util.List;
import java.util.stream.Collectors;

public class StreamBlockStaticOwner {
    public List<Builder> makeAll(List<String> values) {
        return values.stream()
                .map(value -> {
                    Builder builder = Coordinates.GenomicsDBColumn.newBuilder();
                    builder.setName(value);
                    return builder;
                })
                .collect(Collectors.toList());
    }

    static class Coordinates {
        static class GenomicsDBColumn {
            static Builder newBuilder() {
                return new Builder();
            }
        }
    }

    static class Builder {
        String name;

        void setName(String name) {
            this.name = name;
        }
    }
}
