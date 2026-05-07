#import <CoreML/CoreML.h>
#include <stdint.h>
#include <string.h>
#include <stdio.h>

typedef void* CoreMLHandle;

extern "C" {
    CoreMLHandle coreml_load(const char* path);
    CoreMLHandle coreml_load_cpu_only(const char* path);
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

    // @autoreleasepool garantit que tous les objets ObjC créés dans cette
    // inférence (MLMultiArray, MLFeatureValue, MLDictionaryFeatureProvider…)
    // sont libérés avant de retourner à Rust.
    // Sans ce bloc, les objets s'accumulent dans le pool du thread appelant
    // et ne sont drainés qu'à la fin du run loop (jamais, pour un thread Rust).
    float result = 0.0f;
    @autoreleasepool {
        // ── Détecter la shape et le type d'entrée depuis la description du modèle ──
        // Cela permet de supporter aussi bien le modèle mock NeuralNetwork
        // ([1,98,13] Double) que le vrai modèle MIL ([1,1,98,13] Float32).
        MLFeatureDescription* inputDesc =
            model.modelDescription.inputDescriptionsByName[@"mfcc_input"];
        NSArray<NSNumber*>* shape = inputDesc.multiArrayConstraint.shape;
        MLMultiArrayDataType dtype  = inputDesc.multiArrayConstraint.dataType;

        NSError* err = nil;
        MLMultiArray* array = [[MLMultiArray alloc]
                                initWithShape:shape
                                     dataType:dtype
                                        error:&err];
        if (err != nil || array == nil) {
            const char* msg = err ? [err.localizedDescription UTF8String] : "array==nil";
            fprintf(stderr, "[coreml_bridge] MLMultiArray alloc ECHEC — %s\n", msg);
            goto done; // libère le pool puis retourne 0.0f
        }

        {
            // Calculer la taille totale depuis la shape réelle du modèle
            size_t total = 1;
            for (NSNumber* dim in shape) { total *= dim.unsignedIntegerValue; }
            size_t copy_len = (len < total) ? len : total;

            if (dtype == MLMultiArrayDataTypeFloat32) {
                // Copie directe float→float (vrai modèle MIL)
                memcpy(array.dataPointer, mfcc_flat, copy_len * sizeof(float));
            } else {
                // Conversion float→double (modèle NeuralNetwork legacy)
                double* dst = (double*)array.dataPointer;
                for (size_t i = 0; i < copy_len; ++i) {
                    dst[i] = (double)mfcc_flat[i];
                }
            }

            // Construit le FeatureProvider
            MLFeatureValue* fv      = [MLFeatureValue featureValueWithMultiArray:array];
            NSDictionary*   dict    = @{@"mfcc_input": fv};
            id<MLFeatureProvider> input =
                [[MLDictionaryFeatureProvider alloc] initWithDictionary:dict error:&err];
            if (err != nil || input == nil) {
                const char* msg = err ? [err.localizedDescription UTF8String] : "input==nil";
                fprintf(stderr, "[coreml_bridge] FeatureProvider ECHEC — %s\n", msg);
                goto done;
            }

            // Lance l'inférence
            id<MLFeatureProvider> output = [model predictionFromFeatures:input error:&err];
            if (err != nil || output == nil) {
                const char* msg = err ? [err.localizedDescription UTF8String] : "output==nil";
                fprintf(stderr, "[coreml_bridge] prediction ECHEC — %s\n", msg);
                goto done;
            }

            // Récupère classLabel_probs[1] = score wake-word
            MLFeatureValue* outFV = [output featureValueForName:@"classLabel_probs"];
            if (outFV == nil) {
                fprintf(stderr, "[coreml_bridge] 'classLabel_probs' absent des outputs\n");
                goto done;
            }
            MLMultiArray* probs = outFV.multiArrayValue;
            if (probs == nil || probs.count < 2) {
                fprintf(stderr, "[coreml_bridge] tableau de sortie invalide\n");
                goto done;
            }

            result = (float)[probs objectAtIndexedSubscript:1].doubleValue;
        }
        done:;
    } // ← tous les objets ObjC libérés ici

    return result;
}

// ─── coreml_load_cpu_only ─────────────────────────────────────────────────────
// Identique à coreml_load mais force MLComputeUnitsCPUOnly.
// Utilisé pour les benchmarks CPU-only permettant de mesurer le gain ANE.
CoreMLHandle coreml_load_cpu_only(const char* path) {
    if (path == nullptr) {
        fprintf(stderr, "[coreml_bridge] coreml_load_cpu_only: path est nullptr\n");
        return nullptr;
    }

    NSString* modelPath = [NSString stringWithUTF8String:path];
    NSURL*    modelURL  = [NSURL fileURLWithPath:modelPath];

    MLModelConfiguration* config = [[MLModelConfiguration alloc] init];
    config.computeUnits = MLComputeUnitsCPUOnly;

    NSError* err = nil;
    MLModel* model = [MLModel modelWithContentsOfURL:modelURL
                                       configuration:config
                                               error:&err];
    fprintf(stderr, "[coreml_bridge] coreml_load_cpu_only: chargement '%s'\n", path);
    if (err != nil || model == nil) {
        const char* msg = err ? [err.localizedDescription UTF8String] : "model==nil";
        fprintf(stderr, "[coreml_bridge] coreml_load_cpu_only: ECHEC — %s\n", msg);
        return nullptr;
    }

    return (CoreMLHandle)CFBridgingRetain(model);
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
