import java.nio.file.Path;

class NullablePathOverloadArgument {
    void check(Path value) {
        check(value, null);
    }

    void check(Path value, Path optionalIndex) {
        Path index = optionalIndex != null ? optionalIndex : value.resolveSibling("index");
        use(index);
    }

    void use(Path path) {
    }
}
