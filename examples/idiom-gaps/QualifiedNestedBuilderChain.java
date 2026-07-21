package idiom.gaps;

import java.util.Map;

class QualifiedNestedBuilderChain {
    VidMapping update(
            final VidMapping mapping,
            final Map<String, Integer> indexes,
            final String fieldName,
            final String operation) {
        final int index = indexes.containsKey(fieldName) ? indexes.get(fieldName) : -1;
        if (index >= 0) {
            final VidMapping.Builder mappingBuilder = mapping.toBuilder();
            final FieldInfo.Builder fieldBuilder = mappingBuilder.getFieldsBuilder(index);
            fieldBuilder.setCombineOperation(operation);
            return mappingBuilder.build();
        }
        return mapping;
    }
}

class VidMapping {
    Builder toBuilder() {
        return new Builder();
    }

    static class Builder {
        FieldInfo.Builder getFieldsBuilder(int index) {
            return new FieldInfo.Builder();
        }

        VidMapping build() {
            return new VidMapping();
        }
    }
}

class FieldInfo {
    static class Builder {
        void setCombineOperation(String operation) {
        }
    }
}
