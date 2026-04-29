#import <CoreML/CoreML.h>
#include <stdint.h>
#include <string.h>

typedef void* CoreMLHandle;

extern "C" {
    CoreMLHandle coreml_load(const char* path);
    float        coreml_infer(CoreMLHandle handle, const float* mfcc_flat, size_t len);
    void         coreml_free(CoreMLHandle handle);
}

CoreMLHandle coreml_load(const char* /*path*/) {
    return nullptr;
}

float coreml_infer(CoreMLHandle /*handle*/, const float* /*mfcc_flat*/, size_t /*len*/) {
    return 0.0f;
}

void coreml_free(CoreMLHandle /*handle*/) {}
