#import <CoreML/CoreML.h>
#include <stdint.h>
#include <string.h>

typedef void* CoreMLHandle;

extern "C" {
    CoreMLHandle coreml_load(const char* path);
    float        coreml_infer(CoreMLHandle handle, const float* mfcc_flat, size_t len);
    void         coreml_free(CoreMLHandle handle);
}

// ─── coreml_load ─────────────────────────────────────────────────────────────
// Charge le .mlmodelc situé à `path`.
// Retourne un handle ARC-retained (via CFBridgingRetain) ou nullptr.
CoreMLHandle coreml_load(const char* path) {
    if (path == nullptr) {
        NSLog(@"[coreml_bridge] coreml_load: path est nullptr");
        return nullptr;
    }

    NSString* modelPath = [NSString stringWithUTF8String:path];
    NSURL*    modelURL  = [NSURL fileURLWithPath:modelPath];

    MLModelConfiguration* config = [[MLModelConfiguration alloc] init];
    config.computeUnits = MLComputeUnitsAll;

    NSError* err = nil;
    MLModel* model = [MLModel modelWithContentsOfURL:modelURL
                                       configuration:config
                                               error:&err];
    if (err != nil || model == nil) {
        NSLog(@"[coreml_bridge] coreml_load: échec — %@", err.localizedDescription);
        return nullptr;
    }

    // Transfère la propriété ARC vers Rust (CFBridgingRetain +1)
    return (CoreMLHandle)CFBridgingRetain(model);
}

// ─── coreml_infer ────────────────────────────────────────────────────────────
// Lance une inférence synchrone.
// mfcc_flat : len floats représentant la matrice MFCC aplatie [1, 98, 13].
// Retourne le score wake-word (index 1 de classLabel_probs), ou 0.0 si erreur.
float coreml_infer(CoreMLHandle handle, const float* mfcc_flat, size_t len) {
    if (handle == nullptr) {
        NSLog(@"[coreml_bridge] coreml_infer: handle nullptr");
        return 0.0f;
    }
    if (mfcc_flat == nullptr) {
        NSLog(@"[coreml_bridge] coreml_infer: mfcc_flat nullptr");
        return 0.0f;
    }

    MLModel* model = (__bridge MLModel*)handle;

    // Construit MLMultiArray [1, 98, 13] Float32
    NSArray<NSNumber*>* shape = @[@1, @98, @13];
    NSError* err = nil;
    MLMultiArray* array = [[MLMultiArray alloc]
                            initWithShape:shape
                                 dataType:MLMultiArrayDataTypeFloat32
                                    error:&err];
    if (err != nil || array == nil) {
        NSLog(@"[coreml_bridge] coreml_infer: MLMultiArray alloc échoué — %@",
              err.localizedDescription);
        return 0.0f;
    }

    // Copie les floats Rust dans le buffer Core ML
    size_t expected = 1 * 98 * 13;
    size_t copy_len = (len < expected) ? len : expected;
    memcpy(array.dataPointer, mfcc_flat, copy_len * sizeof(float));

    // Construit le FeatureProvider
    MLFeatureValue* fv      = [MLFeatureValue featureValueWithMultiArray:array];
    NSDictionary*   dict    = @{@"mfcc_input": fv};
    id<MLFeatureProvider> input =
        [[MLDictionaryFeatureProvider alloc] initWithDictionary:dict error:&err];
    if (err != nil || input == nil) {
        NSLog(@"[coreml_bridge] coreml_infer: FeatureProvider échoué — %@",
              err.localizedDescription);
        return 0.0f;
    }

    // Lance l'inférence
    id<MLFeatureProvider> output = [model predictionFromFeatures:input error:&err];
    if (err != nil || output == nil) {
        NSLog(@"[coreml_bridge] coreml_infer: prédiction échouée — %@",
              err.localizedDescription);
        return 0.0f;
    }

    // Récupère classLabel_probs[1] = score wake-word
    MLFeatureValue* outFV = [output featureValueForName:@"classLabel_probs"];
    if (outFV == nil) {
        NSLog(@"[coreml_bridge] coreml_infer: 'classLabel_probs' absent des outputs");
        return 0.0f;
    }
    MLMultiArray* probs = outFV.multiArrayValue;
    if (probs == nil || probs.count < 2) {
        NSLog(@"[coreml_bridge] coreml_infer: tableau de sortie invalide");
        return 0.0f;
    }

    return [probs objectAtIndexedSubscript:1].floatValue;
}

// ─── coreml_free ─────────────────────────────────────────────────────────────
// Libère le MLModel ARC retenu par CFBridgingRetain dans coreml_load.
void coreml_free(CoreMLHandle handle) {
    if (handle == nullptr) {
        return;
    }
    // Rend la propriété à ARC (CFBridgingRelease -1) → l'objet est détruit
    CFBridgingRelease(handle);
}
