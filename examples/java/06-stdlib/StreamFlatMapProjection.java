import java.util.List;
import java.util.stream.Collectors;

public class StreamFlatMapProjection {
    List<String> flatten(List<NameGroup> groups) {
        return groups.stream()
                .flatMap(group -> group.names().stream())
                .collect(Collectors.toList());
    }
}

class NameGroup {
    private final List<String> names;

    NameGroup(List<String> names) {
        this.names = names;
    }

    List<String> names() {
        return names;
    }
}
