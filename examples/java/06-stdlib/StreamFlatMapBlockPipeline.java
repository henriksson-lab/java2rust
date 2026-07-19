package examples.stdlib;

import java.util.List;
import java.util.stream.Collectors;

public class StreamFlatMapBlockPipeline {
    public List<String> cleanGroups(List<Group> groups) {
        return groups.stream().flatMap(group -> {
            List<String> names = group.names();
            return names.stream()
                    .filter(name -> !name.isEmpty())
                    .map(name -> name.trim());
        }).collect(Collectors.toList());
    }
}

class Group {
    List<String> names() {
        return null;
    }
}
