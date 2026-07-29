package idiom.gaps;

import java.util.Map;
import java.util.LinkedHashMap;

public class SharedSelfFieldOwnedRecovery {
    String name;
    String workspace;
    String tag;
    Map<String, String> paths;

    public SharedSelfFieldOwnedRecovery(String name, String workspace, Map<String, String> paths) {
        this.name = name;
        this.workspace = workspace;
        this.paths = paths;
    }

    public String getName() {
        return name;
    }

    public String getWorkspace() {
        return this.workspace;
    }

    public Map<String, String> getPaths() {
        return paths;
    }

    public int consumeWorkspace() {
        return length(workspace);
    }

    public Map<String, Object> encodeObjectMap() {
        Map<String, Object> map = new LinkedHashMap<String, Object>();
        map.put("ID", name);
        map.put("Workspace", this.workspace);
        if (tag != null) {
            map.put("Tag", tag);
        }
        map.put("Paths", paths);
        return map;
    }

    private int length(String value) {
        return value.length();
    }
}
