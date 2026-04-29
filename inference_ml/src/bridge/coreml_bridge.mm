#import <CoreML/CoreML.h>
#include <stdint.h>
#include <string.h>
#include <stdio.h>

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
        fprintf(stderr, "[coreml_bridge] coreml_load: path est nullptr\n");
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
    fprintf(stderr, "[coreml_bridge] coreml_load: tentative chargement '%s'\n", path);
    if (err != nil || model == nil) {
        const char* msg = err ? [err.localizedDescription UTF8String] : "model==nil";
        fprintf(stderr, "[coreml_bridge] coreml_load: ECHEC — %s\n", msg);
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
        fprintf(stderr, "[coreml_bridge] coreml_infer: handle nullptr\n");
        return 0.0f;
    }
    if (mfcc_flat == nullptr) {
        fprintf(stderr, "[coreml_bridge] coreml_infer: mfcc_flat nullptr\n");
        return 0.0f;
    }

    MLModel* model = (__bridge MLModel*)handle;

    // Construit MLMultiArray [1, 98, 13] Double
    // (le modèle NeuralNetwork compilé par coremltools utilise Double par défaut)
    NSArray<NSNumber*>* shape = @[@1, @98, @13];
    NSError* err = nil;
    MLMultiArray* array = [[MLMultiArray alloc]
                            initWithShape:shape
                                 dataType:MLMultiArrayDataTypeDouble
                                    error:&err];
    if (err != nil || array == nil) {
        const char* msg = err ? [err.localizedDescription UTF8String] : "array==nil";
        fprintf(stderr, "[coreml_bridge] MLMultiArray alloc ECHEC — %s\n", msg);
        return 0.0f;
    }

    // Convertit les float Rust en double pour CoreML
    size_t expected = 1 * 98 * 13;
    size_t copy_len = (len < expected) ? len : expected;
    double* dst = (double*)array.dataPointer;
    for (size_t i = 0; i < copy_len; ++i) {
        dst[i] = (double)mfcc_flat[i];
    }

    // Construit le FeatureProvider
    MLFeatureValue* fv      = [MLFeatureValue featureValueWithMultiArray:array];
    NSDictionary*   dict    = @{@"mfcc_input": fv};
    id<MLFeatureProvider> input =
        [[MLDictionaryFeatureProvider alloc] initWithDictionary:dict error:&err];
    if (err != nil || input == nil) {
        const char* msg = err ? [err.localizedDescription UTF8String] : "input==nil";
        fprintf(stderr, "[coreml_bridge] FeatureProvider ECHEC — %s\n", msg);
        return 0.0f;
    }

    // Lance l'inférence
    id<MLFeatureProvider> output = [model predictionFromFeatures:input error:&err];
    if (err != nil || output == nil) {
        const char* msg = err ? [err.localizedDescription UTF8String] : "output==nil";
        fprintf(stderr, "[coreml_bridge] prediction ECHEC — %s\n", msg);
        return 0.0f;
    }

    // Récupère classLabel_probs[1] = score wake-word
    MLFeatureValue* outFV = [output featureValueForName:@"classLabel_probs"];
    if (outFV == nil) {
        fprintf(stderr, "[coreml_bridge] 'classLabel_probs' absent des outputs\n");
        return 0.0f;
    }
    MLMultiArray* probs = outFV.multiArrayValue;
    if (probs == nil || probs.count < 2) {
        fprintf(stderr, "[coreml_bridge] tableau de sortie invalide\n");
        return 0.0f;
    }

    return (float)[probs objectAtIndexedSubscript:1].doubleValue;
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
