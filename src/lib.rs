#[macro_use]
extern crate serde;

/**
 * https://cloud.google.com/translate/docs/reference/rest/
 */

use std::collections::HashMap;
use std::io::{self, Write};
use std::result::Result as StdResult;

use futures::{Future, Stream};
use futures::future::{loop_fn, Loop};
use hyper::{Body, Client, Method, Request};
use hyper::header::HeaderValue;
use hyper::rt::{self};
use hyper_tls::HttpsConnector;

use serde::{Serialize, Deserialize, Deserializer};
use serde::de::DeserializeOwned;

#[derive(Debug)]
pub enum Error {
    HyperError(hyper::error::Error),
    SerdeJsonError(serde_json::Error),
    ResponseError(u16, serde_json::Value),
    Other(String),
}

impl From<hyper::error::Error> for Error {
    fn from(e: hyper::error::Error) -> Self {
        Error::HyperError(e)
    }
}

pub type Result<T> = StdResult<T, Error>;

struct Empty;

trait RequestOrEmpty {
    const IS_EMPTY: bool;
    fn to_json(&self) -> String;
}

impl<T> RequestOrEmpty for T where T: Serialize {
    const IS_EMPTY: bool = false;
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

impl RequestOrEmpty for Empty {
    const IS_EMPTY: bool = true;
    fn to_json(&self) -> String {
        unreachable!()
    }
}

trait ParamsOrEmpty {
    const IS_EMPTY: bool;
    fn to_params(&self) -> String;
}

impl<T> ParamsOrEmpty for T where T: Serialize {
    const IS_EMPTY: bool = false;
    fn to_params(&self) -> String {
        serde_urlencoded::to_string(self).unwrap()
    }
}

impl ParamsOrEmpty for Empty {
    const IS_EMPTY: bool = true;
    fn to_params(&self) -> String {
        unreachable!()
    }
}

trait ResponseOrEmpty: Sized {
    const IS_EMPTY: bool;
    fn from_slice(data: &[u8]) -> StdResult<Self, serde_json::Error>;
}

impl<T> ResponseOrEmpty for T where T: DeserializeOwned {
    const IS_EMPTY: bool = false;
    fn from_slice(data: &[u8]) -> StdResult<Self, serde_json::Error> {
        serde_json::from_slice(data)
    }
}

impl ResponseOrEmpty for Empty {
    const IS_EMPTY: bool = true;
    fn from_slice(_data: &[u8]) -> StdResult<Self, serde_json::Error> {
        Ok(Empty)
    }
}

fn post_request<IB, OB>(url: &str, access_token: &str, request_body: &IB) -> impl Future<Item=OB, Error=Error>
    where IB: RequestOrEmpty, OB: ResponseOrEmpty
{
    let mut req = if IB::IS_EMPTY {
        Request::default()
    } else {
        Request::new(Body::from(request_body.to_json()))
    };
    *req.method_mut() = Method::POST;
    *req.uri_mut() = url.parse().unwrap();
    req.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json")
    );
    req.headers_mut().insert(
        hyper::header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", access_token.trim())).unwrap(),
    );
    let https = HttpsConnector::new(4).expect("TLS initialization failed");
    let client = Client::builder()
        .build::<_, hyper::Body>(https);
    client.request(req)
        .and_then(|res| {
            let status = res.status();
            res.into_body().concat2().map(move |body| (status, body))
        })
        .map_err(|err| Error::HyperError(err))
        .and_then(|(status, body)| {
            println!("POST: {}", status);
            if status == code::OK {
                OB::from_slice(body.as_ref()).map_err(|e| Error::SerdeJsonError(e))
            } else {
                match serde_json::from_slice(body.as_ref()) {
                    Ok(body) => Err(Error::ResponseError(status.as_u16(), body)),
                    Err(e) => Err(Error::SerdeJsonError(e)),
                }
            }
        })
}

fn get_request<IB, OB>(url: &str, access_token: &str, params: &IB) -> impl Future<Item=OB, Error=Error>
    where IB: ParamsOrEmpty, OB: ResponseOrEmpty
{
    let url = if IB::IS_EMPTY {
        url.to_string()
    } else {
        format!("{}?{}", url, params.to_params())
    };
    let mut req = Request::default();
    *req.method_mut() = Method::GET;
    *req.uri_mut() = url.parse().unwrap();
    req.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json")
    );
    req.headers_mut().insert(
        hyper::header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", access_token.trim())).unwrap(),
    );
    let https = HttpsConnector::new(4).expect("TLS initialization failed");
    let client = Client::builder()
        .build::<_, hyper::Body>(https);
    client.request(req)
        .and_then(|res| {
            let status = res.status();
            res.into_body().concat2().map(move |body| (status, body))
        })
        .map_err(|err| Error::HyperError(err))
        .and_then(|(status, body)| {
            println!("POST: {}", status);
            if status == code::OK {
                OB::from_slice(body.as_ref()).map_err(|e| Error::SerdeJsonError(e))
            } else {
                match serde_json::from_slice(body.as_ref()) {
                    Ok(body) => Err(Error::ResponseError(status.as_u16(), body)),
                    Err(e) => Err(Error::SerdeJsonError(e)),
                }
            }
        })
}

fn delete_request<OB>(url: &str, access_token: &str) -> impl Future<Item=OB, Error=Error>
    where OB: ResponseOrEmpty
{
    let mut req = Request::new(Body::empty());
    *req.method_mut() = Method::DELETE;
    *req.uri_mut() = url.parse().unwrap();
    req.headers_mut().insert(
        hyper::header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", access_token.trim())).unwrap(),
    );
    let https = HttpsConnector::new(4).expect("TLS initialization failed");
    let client = Client::builder()
        .build::<_, hyper::Body>(https);
    client.request(req)
        .and_then(|res| {
            let status = res.status();
            res.into_body().concat2().map(move |body| (status, body))
        })
        .map_err(|err| Error::HyperError(err))
        .and_then(|(status, body)| {
            println!("POST: {}", status);
            if status == code::OK {
                OB::from_slice(body.as_ref()).map_err(|e| Error::SerdeJsonError(e))
            } else {
                match serde_json::from_slice(body.as_ref()) {
                    Ok(body) => Err(Error::ResponseError(status.as_u16(), body)),
                    Err(e) => Err(Error::SerdeJsonError(e)),
                }
            }
        })
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DetectLanguageRequest {
    /// Optional. The language detection model to be used.
    /// 
    /// Format: projects/{project-id}/locations/{location-id}/models/language-detection/{model-id}
    /// 
    /// Only one language detection model is currently supported:
    /// projects/{project-id}/locations/{location-id}/models/language-detection/default.
    /// 
    /// If not specified, the default model is used.
    /// 
    /// Authorization requires the following [Google IAM](https://cloud.google.com/iam)
    /// permission on the specified resource model:
    /// 
    /// - cloudtranslate.languageDetectionModels.predict
    pub model: Option<String>,
    /// Optional. The format of the source text, for example, "text/html", "text/plain".
    /// If left blank, the MIME type defaults to "text/html".
    pub mime_type: Option<MimeType>,
    /// Optional. The labels with user-defined metadata for the request.
    /// 
    /// Label keys and values can be no longer than 63 characters (Unicode codepoints), can only contain lowercase letters, numeric characters, underscores and dashes. International characters are allowed. Label values are optional. Label keys must start with a letter.
    /// 
    /// See https://goo.gl/xmQnxf for more information on and examples of labels.
    pub labels: Option<HashMap<String, String>>,
    /// The content of the input stored as a string.
    pub content: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
/// The response message for language detection.
pub struct DetectLanguageResponse {
    /// A list of detected languages sorted by detection confidence in descending order. The most probable language first.
    pub languages: Vec<DetectLanguageItem>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
/// The response message for language detection.
pub struct DetectLanguageItem {
    /// The BCP-47 language code of source content in the request, detected automatically.
    pub language_code: String,
    /// The confidence of the detection result for this language.
    pub confidence: f32,
}

/// Detects the language of text within a request.
pub fn detect_language(project_id: &str, location_id: &str, access_token: &str,
        request_body: &DetectLanguageRequest)
    -> impl Future<Item=DetectLanguageResponse, Error=Error> + Send
{
    let url = format!("https://translation.googleapis.com/v3beta1/projects/{}/locations/{}:detectLanguage",
        project_id, location_id);
    post_request(&url, access_token, request_body)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSupportedLanguagesQueryParams {
    /// Optional. The language to use to return localized, human readable names of supported languages.
    /// If missing, then display names are not returned in a response.
    pub display_language_code: Option<String>,
    /// Optional. Get supported languages of this model.
    /// 
    /// The format depends on model type:
    /// 
    /// AutoML Translation models: projects/{project-id}/locations/{location-id}/models/{model-id}
    /// 
    /// General (built-in) models: projects/{project-id}/locations/{location-id}/models/general/nmt, projects/{project-id}/locations/{location-id}/models/general/base
    /// 
    /// Returns languages supported by the specified model. If missing, we get supported languages of Google general base (PBMT) model.
    /// 
    /// Authorization requires one or more of the following [Google IAM](https://cloud.google.com/iam) permissions on the specified resource model:
    /// 
    /// - cloudtranslate.generalModels.get
    /// - automl.models.get
    pub model: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
/// The response message for discovering supported languages.
pub struct SupportedLanguages {
    /// A list of supported language responses. This list contains an entry for each language the Translation API supports.
    pub languages: Vec<SupportedLanguage>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
/// A single supported language response corresponds to information related to one supported language.
pub struct SupportedLanguage {
    /// Supported language code, generally consisting of its ISO 639-1 identifier,
    /// for example, 'en', 'ja'. In certain cases, BCP-47 codes including language and region identifiers are returned (for example, 'zh-TW' and 'zh-CN')
    pub language_code: String,
    /// Human readable name of the language localized in the display language specified in the request.
    pub display_name: Option<String>,
    /// Can be used as source language.
    pub support_source: bool,
    /// Can be used as target language.
    pub support_target: bool,
}

/// Returns a list of supported languages for translation.
pub fn get_supported_languages(project_id: &str, location_id: &str, access_token: &str,
        query_params: &GetSupportedLanguagesQueryParams)
    -> impl Future<Item=SupportedLanguages, Error=Error> + Send
{
    let url = format!("https://translation.googleapis.com/v3beta1/projects/{}/locations/{}/supportedLanguages",
        project_id, location_id);
    get_request(&url, access_token, query_params)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateTextRequest {
    /// Required. The content of the input in string format. We recommend the total content be less than 30k codepoints.
    /// Use locations.batchTranslateText for larger text.
    pub contents: Vec<String>,
    /// Optional. The format of the source text, for example, "text/html", "text/plain".
    /// If left blank, the MIME type defaults to "text/html".
    pub mime_type: Option<MimeType>,
    /// Optional. The BCP-47 language code of the input text if known,
    /// for example, "en-US" or "sr-Latn". Supported language codes are listed in Language Support.
    /// If the source language isn't specified, the API attempts to identify the source language
    /// automatically and returns the source language within the response.
    pub source_language_code: Option<String>,
    /// Required. The BCP-47 language code to use for translation of the input text,
    /// set to one of the language codes listed in Language Support.
    pub target_language_code: String,
    /// Optional. The model type requested for this translation.
    /// 
    /// The format depends on model type:
    ///
    /// - AutoML Translation models: projects/{project-id}/locations/{location-id}/models/{model-id}
    /// - General (built-in) models: projects/{project-id}/locations/{location-id}/models/general/nmt,
    /// projects/{project-id}/locations/{location-id}/models/general/base
    /// 
    /// For global (non-regionalized) requests, use location-id global. For example,
    /// projects/{project-id}/locations/global/models/general/nmt.
    /// 
    /// If missing, the system decides which google base model to use.
    /// 
    /// Authorization requires one or more of the following [Google IAM](https://cloud.google.com/iam)
    /// permissions on the specified resource model:
    /// 
    /// - cloudtranslate.generalModels.predict
    /// - automl.models.predict
    pub model: Option<String>,
    /// Optional. Glossary to be applied. The glossary must be within the same region (have the same
    /// location-id) as the model, otherwise an INVALID_ARGUMENT (400) error is returned.
    /// 
    /// Authorization requires the following [Google IAM](https://cloud.google.com/iam)
    /// permission on the specified resource glossaryConfig:
    /// 
    /// - cloudtranslate.glossaries.predict
    pub glossary_config: Option<TranslateTextGlossaryConfig>,
    /// Optional. The labels with user-defined metadata for the request.
    /// 
    /// Label keys and values can be no longer than 63 characters (Unicode codepoints), can only contain
    /// lowercase letters, numeric characters, underscores and dashes. International characters are allowed.
    /// Label values are optional. Label keys must start with a letter.
    /// 
    /// See https://goo.gl/xmQnxf for more information on and examples of labels.
    pub labels: Option<Vec<HashMap<String, String>>>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
/// Configures which glossary should be used for a specific target language,
/// and defines options for applying that glossary.
pub struct TranslateTextGlossaryConfig {
    /// Required. Specifies the glossary used for this translation.
    /// Use this format: projects/*/locations/*/glossaries/*
    pub glossary: String,
    /// Optional. Indicates match is case-insensitive. Default value is false if missing.
    pub ignore_case: Option<bool>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TranslateTextResponse {
    /// Text translation responses with no glossary applied.
    /// This field has the same length as contents.
    pub translations: Vec<Translation>,
    /// Text translation responses if a glossary is provided in the request.
    /// This can be the same as translations if no terms apply. This field has the same length as contents.
    pub glossary_translations: Option<Vec<Translation>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
/// A single translation response.
pub struct Translation {
    /// Text translated into the target language.
    pub translated_text: String,
    /// Only present when model is present in the request. This is same as model provided in the request.
    pub model: Option<String>,
    /// The BCP-47 language code of source text in the initial request, detected automatically,
    /// if no source language was passed within the initial request. If the source language was passed,
    /// auto-detection of the language does not occur and this field is empty.
    pub detected_language_code: Option<String>,
    /// The glossaryConfig used for this translation.
    pub glossary_config: Option<TranslateTextGlossaryConfig>,
}

/// Translates input text and returns translated text.
pub fn translate_text(project_id: &str, location_id: &str, access_token: &str,
        request_body: &TranslateTextRequest)
    -> impl Future<Item=TranslateTextResponse, Error=Error> + Send
{
    let url = format!("https://translation.googleapis.com/v3beta1/projects/{}/locations/{}:translateText",
        project_id, location_id);
    post_request(&url, access_token, request_body)
}

/// Translates a large volume of text in asynchronous batch mode.
/// 
/// This function provides real-time output as the inputs are being processed.
/// If caller cancels a request, the partial results (for an input file, it's 
/// all or nothing) may still be available on the specified output location.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchTranslateTextRequest {
    /// Required. Source language code.
    pub source_language_code: String,
    /// Required. Specify up to 10 language codes here.
    pub target_language_codes: Vec<String>,
    /// Optional. The models to use for translation.
    /// 
    /// Map's key is target language code. Map's value is model name.
    /// Value can be a built-in general model, or an AutoML Translation model.
    pub models: Option<HashMap<String, String>>,
    /// Required. Input configurations.
    /// 
    /// The total number of files matched should be <= 1000.
    /// The total content size should be <= 100M Unicode codepoints.
    /// The files must use UTF-8 encoding.
    pub input_configs: Vec<BatchTranslateTextInputConfig>,
    /// Required. Output configuration.
    /// 
    /// If 2 input configs match to the same file (that is, same input path),
    /// we don't generate output for duplicate inputs.
    pub output_config: BatchTranslateTextOutputConfig,
    /// Optional. Glossaries to be applied for translation. It's keyed by target language code.
    /// 
    /// Authorization requires the following Google IAM permission on the specified resource glossaries:
    /// 
    /// - cloudtranslate.glossaries.batchPredict
    pub glossaries: Option<HashMap<String, TranslateTextGlossaryConfig>>,
    /// Optional. The labels with user-defined metadata for the request.
    /// 
    /// Label keys and values can be no longer than 63 characters (Unicode codepoints), can only contain lowercase letters,
    /// numeric characters, underscores and dashes. International characters are allowed. Label values are optional. Label keys must start with a letter.
    /// 
    /// See https://goo.gl/xmQnxf for more information on and examples of labels.
    pub labels: Option<HashMap<String, String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchTranslateTextInputConfig {
    /// Optional. Can be "text/plain" or "text/html". For .tsv,
    /// "text/html" is used if mimeType is missing. For .html,
    /// this field must be "text/html" or empty. For .txt, this field must be "text/plain" or empty.
    pub mime_type: Option<MimeType>,
    /// Required. Google Cloud Storage location for the source input. This can be a single file (for example,
    /// gs://translation-test/input.tsv) or a wildcard (for example, gs://translation-test/*). If a file extension is .tsv,
    /// it can contain either one or two columns. The first column (optional) is the id of the text request. If the first column is missing,
    /// we use the row number (0-based) from the input file as the ID in the output file. The second column is the actual text to be translated.
    /// We recommend each row be <= 10K Unicode codepoints, otherwise an error might be returned. Note that the input tsv must be RFC 4180 compliant.
    /// 
    /// You could use https://github.com/Clever/csvlint to check potential formatting errors in your tsv file. csvlint --delimiter='\t' your_input_file.tsv
    /// 
    /// The other supported file extensions are .txt or .html, which is treated as a single large chunk of text.
    pub gcs_source: GcsSource,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
/// Output configuration for locations.batchTranslateText reques
pub struct BatchTranslateTextOutputConfig {
    /// Google Cloud Storage destination for output content. For every single input file (for example, gs://a/b/c.[extension]),
    /// we generate at most 2 * n output files. (n is the # of targetLanguageCodes in the BatchTranslateTextRequest).
    /// 
    /// Output files (tsv) generated are compliant with RFC 4180 except that record delimiters are '\n' instead of '\r\n'.
    /// We don't provide any way to change record delimiters.
    /// 
    /// While the input files are being processed, we write/update an index file 'index.csv' under 'outputUriPrefix' (for example,
    /// gs://translation-test/index.csv) The index file is generated/updated as new files are being translated. The format is:
    /// 
    /// input_file,targetLanguageCode,translations_file,errors_file, glossary_translations_file,glossary_errors_file
    /// 
    /// input_file is one file we matched using gcsSource.input_uri. targetLanguageCode is provided in the request.
    /// translations_file contains the translations. (details provided below) errors_file contains the errors during processing of the file.
    /// (details below). Both translations_file and errors_file could be empty strings if we have no content to output. glossary_translations_file and
    /// glossary_errors_file are always empty strings if the input_file is tsv. They could also be empty if we have no content to output.
    /// 
    /// Once a row is present in index.csv, the input/output matching never changes. Callers should also expect all the content in input_file are
    /// processed and ready to be consumed (that is, no partial output file is written).
    /// 
    /// The format of translations_file (for target language code 'trg') is: gs://translation_test/a_b_c_'trg'_translations.[extension]
    /// 
    /// If the input file extension is tsv, the output has the following columns: Column 1: ID of the request provided in the input,
    /// if it's not provided in the input, then the input row number is used (0-based). Column 2: source sentence. Column 3: translation without
    /// applying a glossary. Empty string if there is an error. Column 4 (only present if a glossary is provided in the request): translation after
    /// applying the glossary. Empty string if there is an error applying the glossary. Could be same string as column 3 if there is no glossary applied.
    /// 
    /// If input file extension is a txt or html, the translation is directly written to the output file. If glossary is requested, a separate
    /// glossary_translations_file has format of gs://translation_test/a_b_c_'trg'_glossary_translations.[extension]
    /// 
    /// The format of errors file (for target language code 'trg') is: gs://translation_test/a_b_c_'trg'_errors.[extension]
    /// 
    /// If the input file extension is tsv, errors_file contains the following: Column 1: ID of the request provided in the input,
    /// if it's not provided in the input, then the input row number is used (0-based). Column 2: source sentence. Column 3: Error detail for the translation.
    /// Could be empty. Column 4 (only present if a glossary is provided in the request): Error when applying the glossary.
    /// 
    /// If the input file extension is txt or html, glossary_error_file will be generated that contains error details. glossary_error_file has format
    /// of gs://translation_test/a_b_c_'trg'_glossary_errors.[extension]
    pub gcs_destination: GcsDestination,
}

#[derive(Serialize, Debug)]
pub enum MimeType {
    #[serde(rename="text/plain")]
    Plain,
    #[serde(rename="text/html")]
    Html,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
/// The Google Cloud Storage location for the input content.
pub struct GcsSource {
    /// Required. Source data URI. For example, gs://my_bucket/my_object.
    pub input_uri: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
/// The Google Cloud Storage location for the output content.
pub struct GcsDestination {
    /// Required. There must be no files under 'outputUriPrefix'.
    /// 'outputUriPrefix' must end with "/" and start with "gs://", otherwise an INVALID_ARGUMENT (400) error is returned.
    pub output_uri_prefix: String,
}

macro_rules! define_error_codes {
    ($($name:ident $http_status_code:tt);*;) => {
        pub mod code {
            $(pub const $name: u16 = $http_status_code);*;
        }
    };
}

// https://cloud.google.com/apis/design/errors
define_error_codes!{
    OK 200;
    CANCELLED 499;
    UNKNOWN 500;
    INVALID_ARGUMENT 400;
    DEADLINE_EXCEEDED 504;
    NOT_FOUND 404;
    ALREADY_EXISTS 409;
    PERMISSION_DENIED 403;
    UNAUTHENTICATED 401;
    RESOURCE_EXHAUSTED 429;
    FAILED_PRECONDITION 400;
    ABORTED 409;
    OUT_OF_RANGE 400;
    UNIMPLEMENTED 501;
    INTERNAL 500;
    UNAVAILABLE 503;
    DATA_LOSS 500;
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
/// This resource represents a long-running operation that is the result of a network API call.
pub struct Operation {
    /// The server-assigned name, which is only unique within the same service that originally returns it.
    /// If you use the default HTTP mapping, the name should be a resource name ending with operations/{unique_id}.
    pub name: String,
    /// Service-specific metadata associated with the operation.
    /// It typically contains progress information and common metadata such as create time.
    /// Some services might not provide such metadata. Any method that returns a long-running operation should document the metadata type, if any.
    /// 
    /// An object containing fields of an arbitrary type. An additional field "@type" contains a URI identifying the type.
    /// Example: { "id": 1234, "@type": "types.example.com/standard/id" }.
    pub metadata: serde_json::Value,
    /// If the value is false, it means the operation is still in progress. If true, the operation is completed, and either error or response is available.
    pub done: Option<bool>,
    /// The error result of the operation in case of failure or cancellation.
    pub error: Option<Status>,
    /// The normal response of the operation in case of success. If the original method returns no data on success, such as Delete, the response
    /// is google.protobuf.Empty. If the original method is standard Get/Create/Update, the response should be the resource. For other methods,
    /// the response should have the type XxxResponse, where Xxx is the original method name. For example, if the original method name is TakeSnapshot(),
    /// the inferred response type is TakeSnapshotResponse.
    /// 
    /// An object containing fields of an arbitrary type. An additional field "@type" contains a URI identifying the type.
    /// Example: { "id": 1234, "@type": "types.example.com/standard/id" }.
    pub response: Option<serde_json::Value>,
}

impl Operation {
    pub fn wait_util_done(&self, access_token: &str) -> impl Future<Item=StdResult<serde_json::Value, Status>, Error=Error> {
        let name = self.name.to_string();
        let access_token = access_token.to_string();
        loop_fn((), move |_| {
            wait_operation(&name, &access_token, &WaitOperationRequestBody { timeout: Some("1s".to_string()) })
            .and_then(|new_operation| {
                match new_operation.done {
                    None | Some(false) => Ok(Loop::Continue(())),
                    Some(true) => {
                        match new_operation.response {
                            Some(response) => Ok(Loop::Break(Ok(response))),
                            _ => {
                                match new_operation.error {
                                    Some(error) => Ok(Loop::Break(Err(error))),
                                    None => Err(Error::Other(format!("wait_operation should return one of response or error : {:?}", new_operation))),
                                }
                            }
                        }
                    }
                }
            })
        })        
    }
}

/// Starts asynchronous cancellation on a long-running operation. The server makes a best effort to cancel the operation, but success is
/// not guaranteed. If the server doesn't support this method, it returns google.rpc.Code.UNIMPLEMENTED. Clients can use Operations.GetOperation
/// or other methods to check whether the cancellation succeeded or whether the operation completed despite cancellation. On successful cancellation,
/// the operation is not deleted; instead, it becomes an operation with an Operation.error value with a google.rpc.Status.code of 1, corresponding to
/// Code.CANCELLED.
fn cancel_operation(name: &str, access_token: &str) -> impl Future<Item=(), Error=Error> + Send {
    let url = format!("https://translation.googleapis.com/v3beta1/{}:cancel", name);
    post_request(&url, access_token, &())
}

/// Deletes a long-running operation. This method indicates that the client is no longer interested in the operation result.
/// It does not cancel the operation. If the server doesn't support this method, it returns google.rpc.Code.UNIMPLEMENTED.
fn delete_operation(name: &str, access_token: &str) -> impl Future<Item=(), Error=Error> + Send {
    let url = format!("https://translation.googleapis.com/v3beta1/{}:cancel", name);
    delete_request::<Empty>(&url, access_token).map(|_| ())
}

/// Gets the latest state of a long-running operation. Clients can use this method to poll the operation
/// result at intervals as recommended by the API service.
fn get_opertion(name: &str, access_token: &str) -> impl Future<Item=Operation, Error=Error> + Send {
    let url = format!("https://translation.googleapis.com/v3beta1/{}", name);
    get_request(&url, access_token, &Empty)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListOperationsQueryParams {
    /// The standard list filter.
    filter: Option<String>,
    /// The standard list page size.
    page_size: Option<usize>,
    /// The standard list page token.
    page_token: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ListOperationsResponse {
    /// A list of operations that matches the specified filter in the request.
    pub operations: Vec<Operation>,
    /// The standard List next-page token.
    pub next_page_token: Option<String>,
}

/// Lists operations that match the specified filter in the request. If the server doesn't support this method, it returns UNIMPLEMENTED.
/// 
/// NOTE: the name binding allows API services to override the binding to use different resource name schemes, such as users/*/operations.
/// To override the binding, API services can add a binding such as "/v1/{name=users/*}/operations" to their service configuration.
/// For backwards compatibility, the default name includes the operations collection id, however overriding users must ensure the name binding
/// is the parent resource, without the operations collection id.
fn list_operations(project_id: &str, location_id: &str, access_token: &str, params: &ListOperationsQueryParams)
    -> impl Future<Item=ListOperationsResponse, Error=Error> + Send
{
    let url = format!("https://translation.googleapis.com/v3beta1/projects/{}/locations/{}/operations", project_id, location_id);
    get_request(&url, access_token, params)
}

#[derive(Serialize)]
#[serde(rename_all="camelCase")]
struct WaitOperationRequestBody {
    /// The maximum duration to wait before timing out. If left blank, the wait will be at most the time permitted by the underlying HTTP/RPC protocol.
    /// If RPC context deadline is also specified, the shorter one will be used.
    /// 
    /// A duration in seconds with up to nine fractional digits, terminated by 's'. Example: "3.5s".
    pub timeout: Option<String>,
}

/// Waits for the specified long-running operation until it is done or reaches at most a specified timeout, returning the latest state.
/// If the operation is already done, the latest state is immediately returned. If the timeout specified is greater than the default HTTP/RPC timeout,
/// the HTTP/RPC timeout is used. If the server does not support this method, it returns google.rpc.Code.UNIMPLEMENTED. Note that this method is on a
/// best-effort basis. It may return the latest state before the specified timeout (including immediately), meaning even an immediate response is no
/// guarantee that the operation is done.
fn wait_operation(name: &str, access_token: &str, request_body: &WaitOperationRequestBody)
    -> impl Future<Item=Operation, Error=Error> + Send
{
    let url = format!("https://translation.googleapis.com/v3beta1/{}:wait", name);
    post_request(&url, access_token, request_body) 
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// The status code, which should be an enum value of google.rpc.Code.
    pub code: i32,
    /// A developer-facing error message, which should be in English. Any user-facing error message should be localized
    /// and sent in the google.rpc.Status.details field, or localized by the client.
    pub message: String,
    /// A list of messages that carry the error details. There is a common set of message types for APIs to use.
    /// 
    /// An object containing fields of an arbitrary type. An additional field "@type" contains a URI identifying the type.
    /// Example: { "id": 1234, "@type": "types.example.com/standard/id" }.
    pub details: Option<Vec<serde_json::Value>>,
}

/// Translates a large volume of text in asynchronous batch mode.
/// 
/// This function provides real-time output as the inputs are being processed. If caller
/// cancels a request, the partial results (for an input file, it's all or nothing) may
/// still be available on the specified output location.
/// 
/// This call returns immediately and you can use google.longrunning.Operation.name to poll the status of the call.
pub fn batch_translate_text(project_id: &str, location_id: &str, access_token: &str,
        request_body: &BatchTranslateTextRequest)
    -> impl Future<Item=Operation, Error=Error> + Send
{
    let url = format!("https://translation.googleapis.com/v3beta1/projects/{}/locations/{}:batchTranslateText",
        project_id, location_id);
    post_request(&url, access_token, request_body)
}

/// Represents a glossary built from user provided data.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Glossary {
    /// Required. The resource name of the glossary.Glossary names have the form
    /// projects/{project-id}/locations/{location-id}/glossaries/{glossary-id}.
    pub name: String,
    /// Required. Provides examples to build the glossary from.
    /// Total glossary must not exceed 10M Unicode codepoints.
    pub input_config: GlossaryInputConfig,
    /// Output only. The number of entries defined in the glossary.
    pub entry_count: Option<usize>,
    /// Output only. When glossaries.create was called.
    /// 
    /// A timestamp in RFC3339 UTC "Zulu" format, accurate to nanoseconds.
    /// Example: "2014-10-02T15:01:23.045123456Z".
    pub submit_time: Option<String>,
    /// Output only. When the glossary creation was finished.
    /// 
    /// A timestamp in RFC3339 UTC "Zulu" format, accurate to nanoseconds.
    /// Example: "2014-10-02T15:01:23.045123456Z".
    pub end_time: Option<String>,
    /// Used with unidirectional glossaries.
    pub language_pair: Option<LanguageCodePair>,
    /// Used with equivalent term set glossaries.
    pub language_codes_set: Option<LanguageCodesSet>,
}

impl Glossary {
    fn new(name: String, input_config: GlossaryInputConfig, language_pair: LanguageCodePair) -> Glossary {
        Glossary {
            name,
            input_config,
            entry_count: None,
            submit_time: None,
            end_time: None,
            language_pair: Some(language_pair),
            language_codes_set: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GlossaryInputConfig {
    pub gcs_source: GcsSource,
}

/// Used with unidirectional glossaries.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LanguageCodePair {
    /// Required. The BCP-47 language code of the input text, for example,
    /// "en-US". Expected to be an exact match for GlossaryTerm.language_code.
    pub source_language_code: String,
    /// Required. The BCP-47 language code for translation output, for example,
    /// "zh-CN". Expected to be an exact match for GlossaryTerm.language_code.
    pub target_language_code: String,
}

/// Used with equivalent term set glossaries.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LanguageCodesSet {
    /// The BCP-47 language code(s) for terms defined in the glossary. All entries are unique.
    /// The list contains at least two entries. Expected to be an exact match for GlossaryTerm.language_code.
    pub language_codes: Vec<String>,
}

/// Creates a glossary and returns the long-running operation. Returns NOT_FOUND, if the project doesn't exist.
pub fn create_glossary(project_id: &str, location_id: &str, access_token: &str, glossary: &Glossary)
    -> impl Future<Item=Operation, Error=Error> + Send
{
    let url = format!("https://translation.googleapis.com/v3beta1/projects/{}/locations/{}/glossaries",
        project_id, location_id);
    post_request(&url, access_token, glossary)
}

/// Deletes a glossary, or cancels glossary construction if the glossary isn't created yet.
/// Returns NOT_FOUND, if the glossary doesn't exist.
pub fn delete_glossary(name: &str, access_token: &str)
    -> impl Future<Item=Operation, Error=Error> + Send
{
    let url = format!("https://translation.googleapis.com/v3beta1/{}", name);
    delete_request(&url, access_token)
}

/// Gets a glossary. Returns NOT_FOUND, if the glossary doesn't exist.
pub fn get_glossary(name: &str, access_token: &str)
    -> impl Future<Item=Operation, Error=Error> + Send
{
    let url = format!("https://translation.googleapis.com/v3beta1/{}", name);
    get_request(&url, access_token, &Empty)
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ListGlossariesQueryParams {
    /// Optional. Requested page size. The server may return fewer glossaries than requested.
    /// If unspecified, the server picks an appropriate default.
    pub page_size: Option<usize>,
    /// Optional. A token identifying a page of results the server should return. Typically,
    /// this is the value of [ListGlossariesResponse.next_page_token] returned from the previous call to
    /// glossaries.list method. The first page is returned if pageTokenis empty or missing.
    pub page_token: Option<String>,
    /// Optional. Filter specifying constraints of a list operation. Filtering is not supported yet, and
    /// the parameter currently has no effect. If missing, no filtering is performed.
    pub filter: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ListGlossariesResponse {
    /// The list of glossaries for a project.
    pub glossaries: Vec<Glossary>,
    /// A token to retrieve a page of results. Pass this value in the [ListGlossariesRequest.page_token] field
    /// in the subsequent call to glossaries.list method to retrieve the next page of results.
    pub next_page_token: Option<String>,
}

/// Lists glossaries in a project. Returns NOT_FOUND, if the project doesn't exist.
pub fn list_glossaries(project_id: &str, location_id: &str, access_token: &str, params: &ListGlossariesQueryParams)
    -> impl Future<Item=ListGlossariesResponse, Error=Error> + Send
{
    let url = format!("https://translation.googleapis.com/v3beta1/projects/{}/locations/{}/glossaries",
        project_id, location_id);
    get_request(&url, access_token, params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use futures::future::{loop_fn, Loop};

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn test_detect_language() {
        let project_id = std::env::var("PROJECT_ID").unwrap();
        let location_id = std::env::var("LOCATION_ID").unwrap();
        let access_token = std::env::var("ACCESS_TOKEN").unwrap();
        let request_body = DetectLanguageRequest {
            model: None,
            mime_type: None,
            labels: None,
            content: "我是谁是我".to_string(),
        };
        tokio::runtime::current_thread::block_on_all(hyper::rt::lazy(move || {
            detect_language(&project_id, &location_id, &access_token, &request_body)
            .map(|response_body| {
                println!("{:?}", response_body);
            })
        })).unwrap();
    }

    #[test]
    fn test_get_supported_languages() {
        let project_id = std::env::var("PROJECT_ID").unwrap();
        let location_id = std::env::var("LOCATION_ID").unwrap();
        let access_token = std::env::var("ACCESS_TOKEN").unwrap();
        let query_params = GetSupportedLanguagesQueryParams {
            display_language_code: None,
            model: None,
        };
        tokio::runtime::current_thread::block_on_all(hyper::rt::lazy(move || {
            get_supported_languages(&project_id, &location_id, &access_token, &query_params)
            .map(|response_body| {
                println!("{:?}", response_body);
            })
            .map_err(|e| {
                panic!("{:?}", e);
            })
        })).unwrap();
    }

    #[test]
    fn test_translate_text() {
        let project_id = std::env::var("PROJECT_ID").unwrap();
        let location_id = std::env::var("LOCATION_ID").unwrap();
        let access_token = std::env::var("ACCESS_TOKEN").unwrap();
        let glossary_id = std::env::var("GLOSSARY_ID").unwrap();
        let glossary = format!("projects/{}/locations/{}/glossaries/{}", project_id, location_id, glossary_id);
        let request = TranslateTextRequest {
            contents: vec!["player".to_string()],
            mime_type: None,
            labels: None,
            glossary_config: Some(TranslateTextGlossaryConfig {
                glossary,
                ignore_case: Some(true),
            }),
            source_language_code: Some("en".to_string()),
            target_language_code: "zh".to_string(),
            model: None,
        };
        tokio::runtime::current_thread::block_on_all(hyper::rt::lazy(move || {
            translate_text(&project_id, &location_id, &access_token, &request)
            .map(|response_body| {
                println!("{:?}", response_body);
            })
            .map_err(|e| {
                panic!("{:?}", e);
            })
        })).unwrap();
    }

    #[test]
    #[ignore]
    fn test_batch_translate_text() {
        let project_id = std::env::var("PROJECT_ID").unwrap();
        let location_id = std::env::var("LOCATION_ID").unwrap();
        let access_token = std::env::var("ACCESS_TOKEN").unwrap();
        let glossary_id = std::env::var("GLOSSARY_ID").unwrap();
        let glossary = format!("projects/{}/locations/{}/glossaries/{}", project_id, location_id, glossary_id);
        let request = BatchTranslateTextRequest {
            source_language_code: "en".to_string(),
            target_language_codes: vec!["zh".to_string()],
            models: None,
            input_configs: vec![
                BatchTranslateTextInputConfig {
                    gcs_source: GcsSource {
                        input_uri: "gs://mb_input/test.tsv".to_string(),
                    },
                    mime_type: Some(MimeType::Plain),
                }
            ],
            output_config: BatchTranslateTextOutputConfig {
                gcs_destination: GcsDestination {
                    output_uri_prefix: "gs://mb_output/".to_string(),
                }
            },
            glossaries: Some(std::iter::once(("zh".to_string(), TranslateTextGlossaryConfig {
                glossary,
                ignore_case: Some(true),
            })).collect()),
            labels: None,
        };
        tokio::runtime::current_thread::block_on_all(hyper::rt::lazy(move || {
            batch_translate_text(&project_id, &location_id, &access_token, &request)
            .and_then(move |operation| {
                operation.wait_util_done(&access_token).map(|r| {
                    match r {
                        Ok(_) => (),
                        Err(e) => panic!("wait_operation error: {:?}", e),
                    }
                })
            })
            .map_err(|e| {
                panic!("{:?}", e);
            })
        })).unwrap();
    }

    #[test]
    fn test_list_operations() {
        let project_id = std::env::var("PROJECT_ID").unwrap();
        let location_id = std::env::var("LOCATION_ID").unwrap();
        let access_token = std::env::var("ACCESS_TOKEN").unwrap();
        let params = ListOperationsQueryParams {
            filter: None,
            page_size: None,
            page_token: None,
        };
        tokio::runtime::current_thread::block_on_all(hyper::rt::lazy(move || {
            list_operations(&project_id, &location_id, &access_token, &params)
            .map(|list_operations| {
                println!("{:?}", list_operations);
                
            })
            .map_err(|e| {
                panic!("{:?}", e);
            })
        })).unwrap();
    }

    #[test]
    fn test_list_glossaries() {
        let project_id = std::env::var("PROJECT_ID").unwrap();
        let location_id = std::env::var("LOCATION_ID").unwrap();
        let access_token = std::env::var("ACCESS_TOKEN").unwrap();
        let params = ListGlossariesQueryParams {
            filter: None,
            page_size: None,
            page_token: None,
        };
        tokio::runtime::current_thread::block_on_all(hyper::rt::lazy(move || {
            list_glossaries(&project_id, &location_id, &access_token, &params)
            .map(|list_glossaries_response| {
                println!("{:?}", list_glossaries_response);
                
            })
            .map_err(|e| {
                panic!("{:?}", e);
            })
        })).unwrap();
    }

    #[test]
    #[ignore]
    fn test_glossaries() {
        let project_id = Rc::new(std::env::var("PROJECT_ID").unwrap());
        let location_id = Rc::new(std::env::var("LOCATION_ID").unwrap());
        let access_token = Rc::new(std::env::var("ACCESS_TOKEN").unwrap());
        let glossary_bucket_id = Rc::new(std::env::var("GLOSSARY_BUCKET_ID").unwrap());
        let test_glossary_name = format!("projects/{}/locations/{}/glossaries/test", project_id, location_id);
        let test_glossary_gs = format!("gs://{}/test.tsv", glossary_bucket_id);
        tokio::runtime::current_thread::block_on_all(hyper::rt::lazy(move || {
            let access_token2 = access_token.clone();
            let access_token3 = access_token.clone();
            delete_glossary(&test_glossary_name, &access_token)
            .then(move |r| -> Box<dyn Future<Item=(), Error=Error>> {
                match r {
                    Ok(operation) => {
                        println!("{:?}", operation);
                        Box::new(operation.wait_util_done(&access_token2).map(|r| {
                            match r {
                                Ok(_) => (),
                                Err(e) => panic!("wait_operation error: {:?}", e),
                            }
                        }))
                    },
                    Err(Error::ResponseError(code, _)) if code == code::NOT_FOUND => {
                        // nothing to do
                        Box::new(futures::future::ok::<(), Error>(()))
                    },
                    Err(e) => panic!("{:?}", e),
                }
            })
            .map_err(|e| {
                panic!("{:?}", e);
            })
            .and_then(move |_| {
                let glossary = Glossary::new(
                    test_glossary_name,
                    GlossaryInputConfig { gcs_source: GcsSource { input_uri: test_glossary_gs }},
                    LanguageCodePair { source_language_code: "en".to_string(), target_language_code: "zh".to_string()}
                );
                create_glossary(&project_id, &location_id, &access_token, &glossary)
            })
            .and_then(move |operation| {
                println!("{:?}", operation);
                operation.wait_util_done(&access_token3).map(|r| {
                    match r {
                        Ok(_) => (),
                        Err(e) => panic!("wait_operation error: {:?}", e),
                    }
                })
            })
            .map_err(|e| {
                panic!("{:?}", e);
            })
        })).unwrap();
    }

    #[test]
    #[ignore]
    fn test_serde() {
        #[derive(Serialize)]
        struct S {
            name: Option<String>,
        }
        let s = S { name: None };
        let s = serde_json::to_string(&s).unwrap();
        dbg!(s);
        let a: () = serde_json::from_str("").unwrap();
        dbg!(a);
    }
}
