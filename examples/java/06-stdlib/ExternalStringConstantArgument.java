package examples.stdlib;

public class ExternalStringConstantArgument {
    public String useExternalConstant() {
        return accept(GATKVCFConstants.RAW_MAPPING_QUALITY_WITH_DEPTH_KEY);
    }

    public String useExternalConstantWithoutSuffix() {
        return accept(GATKVCFConstants.TREE_SCORE);
    }

    private String accept(String key) {
        return key;
    }
}
