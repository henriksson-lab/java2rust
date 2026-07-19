package idiom.gaps;

import java.util.Map;

public class SharedSelfFieldOwnedRecovery {
    String name;
    String workspace;
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

    private int length(String value) {
        return value.length();
    }
}
