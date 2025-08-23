use crate::couchdb::{
    types::{Document, TensorOperation, TensorOpType},
    error::{CouchError, CouchResult},
    database::DatabaseInstance,
};
use ndarray::{Array1, Array2, ArrayD, Dimension, IxDyn, s};
use ndarray_linalg::{Solve, Inverse, Eig, SVD, QR};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use serde_json::{Value, json};
use log::{info, warn, error, debug};
use uuid::Uuid;
use chrono::Utc;

/// Tensor operations engine for advanced data processing
pub struct TensorEngine {
    operations: Arc<RwLock<HashMap<String, TensorOperation>>>,
    cache: Arc<RwLock<HashMap<String, TensorData>>>,
    config: TensorConfig,
}

/// Tensor configuration
#[derive(Debug, Clone)]
pub struct TensorConfig {
    pub max_cache_size: usize,
    pub enable_gpu: bool,
    pub max_tensor_size: usize,
    pub precision: TensorPrecision,
    pub enable_broadcasting: bool,
}

/// Tensor precision settings
#[derive(Debug, Clone)]
pub enum TensorPrecision {
    Float32,
    Float64,
    Complex64,
    Complex128,
}

impl Default for TensorConfig {
    fn default() -> Self {
        Self {
            max_cache_size: 1000,
            enable_gpu: false, // CPU only for this implementation
            max_tensor_size: 1_000_000, // 1M elements max
            precision: TensorPrecision::Float64,
            enable_broadcasting: true,
        }
    }
}

/// Internal tensor data representation
#[derive(Debug, Clone)]
pub struct TensorData {
    pub id: String,
    pub shape: Vec<usize>,
    pub data: ArrayD<f64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub metadata: HashMap<String, Value>,
}

/// Tensor operation result
#[derive(Debug, Clone)]
pub struct TensorResult {
    pub operation_id: String,
    pub result_tensor: Option<TensorData>,
    pub scalar_result: Option<f64>,
    pub metadata: HashMap<String, Value>,
    pub execution_time_ms: u64,
}

impl TensorEngine {
    /// Create a new tensor engine
    pub fn new(config: TensorConfig) -> Self {
        Self {
            operations: Arc::new(RwLock::new(HashMap::new())),
            cache: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }
    
    /// Load tensor data from document
    pub fn load_tensor_from_document(&self, doc: &Document) -> CouchResult<TensorData> {
        debug!("Loading tensor from document: {}", doc.id);
        
        // Extract tensor metadata
        let tensor_meta = doc.data.get("tensor")
            .ok_or_else(|| CouchError::bad_request("Document does not contain tensor data"))?;
        
        let shape: Vec<usize> = tensor_meta.get("shape")
            .and_then(|s| s.as_array())
            .ok_or_else(|| CouchError::bad_request("Invalid tensor shape"))?
            .iter()
            .map(|v| v.as_u64().unwrap_or(0) as usize)
            .collect();
        
        let data_array = tensor_meta.get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| CouchError::bad_request("Invalid tensor data"))?;
        
        // Convert JSON array to ndarray
        let flat_data: Vec<f64> = data_array.iter()
            .map(|v| v.as_f64().unwrap_or(0.0))
            .collect();
        
        // Validate size
        let expected_size: usize = shape.iter().product();
        if flat_data.len() != expected_size {
            return Err(CouchError::bad_request("Tensor data size does not match shape"));
        }
        
        if expected_size > self.config.max_tensor_size {
            return Err(CouchError::bad_request("Tensor exceeds maximum size limit"));
        }
        
        // Create ndarray
        let ndarray_data = ArrayD::from_shape_vec(IxDyn(&shape), flat_data)
            .map_err(|e| CouchError::bad_request(&format!("Failed to create tensor: {}", e)))?;
        
        let tensor = TensorData {
            id: doc.id.clone(),
            shape,
            data: ndarray_data,
            created_at: Utc::now(),
            metadata: tensor_meta.get("metadata")
                .and_then(|m| m.as_object())
                .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default(),
        };
        
        // Cache the tensor
        let mut cache = self.cache.write().unwrap();
        if cache.len() >= self.config.max_cache_size {
            // Remove oldest entry
            let oldest_key = cache.keys().next().cloned();
            if let Some(key) = oldest_key {
                cache.remove(&key);
            }
        }
        cache.insert(doc.id.clone(), tensor.clone());
        
        info!("Loaded tensor: {} with shape: {:?}", doc.id, tensor.shape);
        Ok(tensor)
    }
    
    /// Store tensor data as document
    pub fn store_tensor_as_document(&self, tensor: &TensorData, doc_id: Option<String>) -> CouchResult<Document> {
        let id = doc_id.unwrap_or_else(|| format!("tensor_{}", Uuid::new_v4()));
        
        // Convert ndarray to JSON
        let flat_data: Vec<f64> = tensor.data.iter().cloned().collect();
        let data_json: Vec<Value> = flat_data.into_iter().map(|f| json!(f)).collect();
        
        let tensor_doc = json!({
            "type": "tensor",
            "tensor": {
                "shape": tensor.shape,
                "data": data_json,
                "metadata": tensor.metadata
            },
            "created_at": tensor.created_at.to_rfc3339()
        });
        
        Ok(Document {
            id,
            rev: "1-new".to_string(),
            deleted: None,
            attachments: None,
            data: tensor_doc,
        })
    }
    
    /// Execute tensor operation
    pub fn execute_operation(&self, operation: &TensorOperation, db: &DatabaseInstance) -> CouchResult<TensorResult> {
        let start_time = std::time::Instant::now();
        let operation_id = Uuid::new_v4().to_string();
        
        info!("Executing tensor operation: {:?}", operation.operation);
        
        // Load input tensors
        let mut input_tensors = Vec::new();
        for doc_id in &operation.input_docs {
            let doc = db.get_document(doc_id)?;
            let tensor = self.load_tensor_from_document(&doc)?;
            input_tensors.push(tensor);
        }
        
        // Execute the operation
        let result = match operation.operation {
            TensorOpType::MatrixMultiply => self.matrix_multiply(&input_tensors, &operation.parameters)?,
            TensorOpType::VectorAdd => self.vector_add(&input_tensors, &operation.parameters)?,
            TensorOpType::VectorSubtract => self.vector_subtract(&input_tensors, &operation.parameters)?,
            TensorOpType::DotProduct => self.dot_product(&input_tensors, &operation.parameters)?,
            TensorOpType::CrossProduct => self.cross_product(&input_tensors, &operation.parameters)?,
            TensorOpType::Transpose => self.transpose(&input_tensors, &operation.parameters)?,
            TensorOpType::Inverse => self.inverse(&input_tensors, &operation.parameters)?,
            TensorOpType::Eigenvalues => self.eigenvalues(&input_tensors, &operation.parameters)?,
            TensorOpType::Svd => self.svd(&input_tensors, &operation.parameters)?,
            TensorOpType::Qr => self.qr(&input_tensors, &operation.parameters)?,
            TensorOpType::Custom(ref name) => self.custom_operation(name, &input_tensors, &operation.parameters)?,
        };
        
        let execution_time = start_time.elapsed().as_millis() as u64;
        
        // Store operation in history
        let mut operations = self.operations.write().unwrap();
        operations.insert(operation_id.clone(), operation.clone());
        
        info!("Tensor operation completed in {}ms", execution_time);
        
        Ok(TensorResult {
            operation_id,
            result_tensor: result.0,
            scalar_result: result.1,
            metadata: HashMap::new(),
            execution_time_ms: execution_time,
        })
    }
    
    /// Matrix multiplication
    fn matrix_multiply(&self, tensors: &[TensorData], _params: &HashMap<String, Value>) -> CouchResult<(Option<TensorData>, Option<f64>)> {
        if tensors.len() != 2 {
            return Err(CouchError::bad_request("Matrix multiplication requires exactly 2 tensors"));
        }
        
        let a = &tensors[0].data;
        let b = &tensors[1].data;
        
        // Ensure we have 2D matrices
        if a.ndim() != 2 || b.ndim() != 2 {
            return Err(CouchError::bad_request("Matrix multiplication requires 2D tensors"));
        }
        
        let a_matrix = a.view().into_dimensionality::<ndarray::Ix2>()
            .map_err(|e| CouchError::bad_request(&format!("Invalid matrix A: {}", e)))?;
        let b_matrix = b.view().into_dimensionality::<ndarray::Ix2>()
            .map_err(|e| CouchError::bad_request(&format!("Invalid matrix B: {}", e)))?;
        
        let result = a_matrix.dot(&b_matrix);
        let result_shape = result.shape().to_vec();
        let result_data = result.into_dyn();
        
        let tensor_result = TensorData {
            id: format!("matmul_{}", Uuid::new_v4()),
            shape: result_shape,
            data: result_data,
            created_at: Utc::now(),
            metadata: HashMap::new(),
        };
        
        Ok((Some(tensor_result), None))
    }
    
    /// Vector addition
    fn vector_add(&self, tensors: &[TensorData], _params: &HashMap<String, Value>) -> CouchResult<(Option<TensorData>, Option<f64>)> {
        if tensors.len() < 2 {
            return Err(CouchError::bad_request("Vector addition requires at least 2 tensors"));
        }
        
        let mut result = tensors[0].data.clone();
        
        for tensor in &tensors[1..] {
            if tensor.shape != tensors[0].shape {
                return Err(CouchError::bad_request("All tensors must have the same shape for addition"));
            }
            result = result + &tensor.data;
        }
        
        let tensor_result = TensorData {
            id: format!("add_{}", Uuid::new_v4()),
            shape: tensors[0].shape.clone(),
            data: result,
            created_at: Utc::now(),
            metadata: HashMap::new(),
        };
        
        Ok((Some(tensor_result), None))
    }
    
    /// Vector subtraction
    fn vector_subtract(&self, tensors: &[TensorData], _params: &HashMap<String, Value>) -> CouchResult<(Option<TensorData>, Option<f64>)> {
        if tensors.len() != 2 {
            return Err(CouchError::bad_request("Vector subtraction requires exactly 2 tensors"));
        }
        
        if tensors[0].shape != tensors[1].shape {
            return Err(CouchError::bad_request("Tensors must have the same shape for subtraction"));
        }
        
        let result = &tensors[0].data - &tensors[1].data;
        
        let tensor_result = TensorData {
            id: format!("sub_{}", Uuid::new_v4()),
            shape: tensors[0].shape.clone(),
            data: result,
            created_at: Utc::now(),
            metadata: HashMap::new(),
        };
        
        Ok((Some(tensor_result), None))
    }
    
    /// Dot product
    fn dot_product(&self, tensors: &[TensorData], _params: &HashMap<String, Value>) -> CouchResult<(Option<TensorData>, Option<f64>)> {
        if tensors.len() != 2 {
            return Err(CouchError::bad_request("Dot product requires exactly 2 tensors"));
        }
        
        // Ensure vectors (1D)
        if tensors[0].data.ndim() != 1 || tensors[1].data.ndim() != 1 {
            return Err(CouchError::bad_request("Dot product requires 1D tensors (vectors)"));
        }
        
        let a = tensors[0].data.view().into_dimensionality::<ndarray::Ix1>()
            .map_err(|e| CouchError::bad_request(&format!("Invalid vector A: {}", e)))?;
        let b = tensors[1].data.view().into_dimensionality::<ndarray::Ix1>()
            .map_err(|e| CouchError::bad_request(&format!("Invalid vector B: {}", e)))?;
        
        let result = a.dot(&b);
        
        Ok((None, Some(result)))
    }
    
    /// Cross product (3D vectors only)
    fn cross_product(&self, tensors: &[TensorData], _params: &HashMap<String, Value>) -> CouchResult<(Option<TensorData>, Option<f64>)> {
        if tensors.len() != 2 {
            return Err(CouchError::bad_request("Cross product requires exactly 2 tensors"));
        }
        
        // Ensure 3D vectors
        if tensors[0].shape != vec![3] || tensors[1].shape != vec![3] {
            return Err(CouchError::bad_request("Cross product requires 3D vectors"));
        }
        
        let a = tensors[0].data.view().into_dimensionality::<ndarray::Ix1>().unwrap();
        let b = tensors[1].data.view().into_dimensionality::<ndarray::Ix1>().unwrap();
        
        let result = Array1::from(vec![
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]).into_dyn();
        
        let tensor_result = TensorData {
            id: format!("cross_{}", Uuid::new_v4()),
            shape: vec![3],
            data: result,
            created_at: Utc::now(),
            metadata: HashMap::new(),
        };
        
        Ok((Some(tensor_result), None))
    }
    
    /// Matrix transpose
    fn transpose(&self, tensors: &[TensorData], _params: &HashMap<String, Value>) -> CouchResult<(Option<TensorData>, Option<f64>)> {
        if tensors.len() != 1 {
            return Err(CouchError::bad_request("Transpose requires exactly 1 tensor"));
        }
        
        let tensor = &tensors[0];
        if tensor.data.ndim() != 2 {
            return Err(CouchError::bad_request("Transpose requires a 2D tensor"));
        }
        
        let matrix = tensor.data.view().into_dimensionality::<ndarray::Ix2>()
            .map_err(|e| CouchError::bad_request(&format!("Invalid matrix: {}", e)))?;
        
        let result = matrix.t().to_owned().into_dyn();
        let mut result_shape = tensor.shape.clone();
        result_shape.reverse(); // Transpose dimensions
        
        let tensor_result = TensorData {
            id: format!("transpose_{}", Uuid::new_v4()),
            shape: result_shape,
            data: result,
            created_at: Utc::now(),
            metadata: HashMap::new(),
        };
        
        Ok((Some(tensor_result), None))
    }
    
    /// Matrix inverse
    fn inverse(&self, tensors: &[TensorData], _params: &HashMap<String, Value>) -> CouchResult<(Option<TensorData>, Option<f64>)> {
        if tensors.len() != 1 {
            return Err(CouchError::bad_request("Inverse requires exactly 1 tensor"));
        }
        
        let tensor = &tensors[0];
        if tensor.data.ndim() != 2 {
            return Err(CouchError::bad_request("Inverse requires a 2D tensor"));
        }
        
        let matrix = tensor.data.view().into_dimensionality::<ndarray::Ix2>()
            .map_err(|e| CouchError::bad_request(&format!("Invalid matrix: {}", e)))?;
        
        // Check if matrix is square
        if matrix.nrows() != matrix.ncols() {
            return Err(CouchError::bad_request("Matrix must be square for inversion"));
        }
        
        let result = matrix.inv()
            .map_err(|e| CouchError::bad_request(&format!("Matrix is not invertible: {}", e)))?;
        
        let tensor_result = TensorData {
            id: format!("inverse_{}", Uuid::new_v4()),
            shape: tensor.shape.clone(),
            data: result.into_dyn(),
            created_at: Utc::now(),
            metadata: HashMap::new(),
        };
        
        Ok((Some(tensor_result), None))
    }
    
    /// Eigenvalue decomposition
    fn eigenvalues(&self, tensors: &[TensorData], _params: &HashMap<String, Value>) -> CouchResult<(Option<TensorData>, Option<f64>)> {
        if tensors.len() != 1 {
            return Err(CouchError::bad_request("Eigenvalue decomposition requires exactly 1 tensor"));
        }
        
        let tensor = &tensors[0];
        if tensor.data.ndim() != 2 {
            return Err(CouchError::bad_request("Eigenvalue decomposition requires a 2D tensor"));
        }
        
        let matrix = tensor.data.view().into_dimensionality::<ndarray::Ix2>()
            .map_err(|e| CouchError::bad_request(&format!("Invalid matrix: {}", e)))?;
        
        let (eigenvals, _eigenvecs) = matrix.eig()
            .map_err(|e| CouchError::bad_request(&format!("Eigenvalue decomposition failed: {}", e)))?;
        
        // Return only eigenvalues for simplicity
        let eigenvals_real: Vec<f64> = eigenvals.iter().map(|c| c.re).collect();
        let result = Array1::from(eigenvals_real).into_dyn();
        
        let tensor_result = TensorData {
            id: format!("eigenvals_{}", Uuid::new_v4()),
            shape: vec![matrix.nrows()],
            data: result,
            created_at: Utc::now(),
            metadata: HashMap::new(),
        };
        
        Ok((Some(tensor_result), None))
    }
    
    /// Singular Value Decomposition
    fn svd(&self, tensors: &[TensorData], _params: &HashMap<String, Value>) -> CouchResult<(Option<TensorData>, Option<f64>)> {
        if tensors.len() != 1 {
            return Err(CouchError::bad_request("SVD requires exactly 1 tensor"));
        }
        
        let tensor = &tensors[0];
        if tensor.data.ndim() != 2 {
            return Err(CouchError::bad_request("SVD requires a 2D tensor"));
        }
        
        let matrix = tensor.data.view().into_dimensionality::<ndarray::Ix2>()
            .map_err(|e| CouchError::bad_request(&format!("Invalid matrix: {}", e)))?;
        
        let (_u, s, _vt) = matrix.svd(true, true)
            .map_err(|e| CouchError::bad_request(&format!("SVD failed: {}", e)))?;
        
        // Return singular values
        let result = Array1::from(s).into_dyn();
        
        let tensor_result = TensorData {
            id: format!("svd_{}", Uuid::new_v4()),
            shape: vec![result.len()],
            data: result,
            created_at: Utc::now(),
            metadata: HashMap::new(),
        };
        
        Ok((Some(tensor_result), None))
    }
    
    /// QR decomposition
    fn qr(&self, tensors: &[TensorData], _params: &HashMap<String, Value>) -> CouchResult<(Option<TensorData>, Option<f64>)> {
        if tensors.len() != 1 {
            return Err(CouchError::bad_request("QR decomposition requires exactly 1 tensor"));
        }
        
        let tensor = &tensors[0];
        if tensor.data.ndim() != 2 {
            return Err(CouchError::bad_request("QR decomposition requires a 2D tensor"));
        }
        
        let matrix = tensor.data.view().into_dimensionality::<ndarray::Ix2>()
            .map_err(|e| CouchError::bad_request(&format!("Invalid matrix: {}", e)))?;
        
        let (q, r) = matrix.qr()
            .map_err(|e| CouchError::bad_request(&format!("QR decomposition failed: {}", e)))?;
        
        // Return Q matrix for simplicity
        let tensor_result = TensorData {
            id: format!("qr_q_{}", Uuid::new_v4()),
            shape: tensor.shape.clone(),
            data: q.into_dyn(),
            created_at: Utc::now(),
            metadata: json!({"decomposition": "Q_matrix"}).as_object().unwrap().clone(),
        };
        
        Ok((Some(tensor_result), None))
    }
    
    /// Custom operation placeholder
    fn custom_operation(&self, name: &str, tensors: &[TensorData], params: &HashMap<String, Value>) -> CouchResult<(Option<TensorData>, Option<f64>)> {
        match name {
            "norm" => self.vector_norm(tensors, params),
            "mean" => self.tensor_mean(tensors, params),
            "std" => self.tensor_std(tensors, params),
            _ => Err(CouchError::bad_request(&format!("Unknown custom operation: {}", name))),
        }
    }
    
    /// Vector/matrix norm
    fn vector_norm(&self, tensors: &[TensorData], _params: &HashMap<String, Value>) -> CouchResult<(Option<TensorData>, Option<f64>)> {
        if tensors.len() != 1 {
            return Err(CouchError::bad_request("Norm requires exactly 1 tensor"));
        }
        
        let data = &tensors[0].data;
        let norm = data.iter().map(|x| x * x).sum::<f64>().sqrt();
        
        Ok((None, Some(norm)))
    }
    
    /// Tensor mean
    fn tensor_mean(&self, tensors: &[TensorData], _params: &HashMap<String, Value>) -> CouchResult<(Option<TensorData>, Option<f64>)> {
        if tensors.len() != 1 {
            return Err(CouchError::bad_request("Mean requires exactly 1 tensor"));
        }
        
        let data = &tensors[0].data;
        let mean = data.iter().sum::<f64>() / data.len() as f64;
        
        Ok((None, Some(mean)))
    }
    
    /// Tensor standard deviation
    fn tensor_std(&self, tensors: &[TensorData], _params: &HashMap<String, Value>) -> CouchResult<(Option<TensorData>, Option<f64>)> {
        if tensors.len() != 1 {
            return Err(CouchError::bad_request("Standard deviation requires exactly 1 tensor"));
        }
        
        let data = &tensors[0].data;
        let mean = data.iter().sum::<f64>() / data.len() as f64;
        let variance = data.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / data.len() as f64;
        let std = variance.sqrt();
        
        Ok((None, Some(std)))
    }
    
    /// Get tensor engine statistics
    pub fn get_stats(&self) -> HashMap<String, Value> {
        let operations = self.operations.read().unwrap();
        let cache = self.cache.read().unwrap();
        
        let mut stats = HashMap::new();
        stats.insert("total_operations".to_string(), json!(operations.len()));
        stats.insert("cached_tensors".to_string(), json!(cache.len()));
        stats.insert("max_cache_size".to_string(), json!(self.config.max_cache_size));
        stats.insert("max_tensor_size".to_string(), json!(self.config.max_tensor_size));
        stats.insert("precision".to_string(), json!(format!("{:?}", self.config.precision)));
        
        let total_cached_elements: usize = cache.values()
            .map(|t| t.data.len())
            .sum();
        stats.insert("total_cached_elements".to_string(), json!(total_cached_elements));
        
        stats
    }
    
    /// Clear tensor cache
    pub fn clear_cache(&self) {
        let mut cache = self.cache.write().unwrap();
        cache.clear();
        info!("Cleared tensor cache");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::couchdb::types::Document;
    
    fn create_test_tensor_document(id: &str, shape: Vec<usize>, data: Vec<f64>) -> Document {
        let data_json: Vec<Value> = data.into_iter().map(|f| json!(f)).collect();
        
        Document {
            id: id.to_string(),
            rev: "1-abc123".to_string(),
            deleted: None,
            attachments: None,
            data: json!({
                "type": "tensor",
                "tensor": {
                    "shape": shape,
                    "data": data_json,
                    "metadata": {}
                }
            }),
        }
    }
    
    #[test]
    fn test_tensor_engine_creation() {
        let config = TensorConfig::default();
        let engine = TensorEngine::new(config);
        
        let stats = engine.get_stats();
        assert_eq!(stats["total_operations"], json!(0));
        assert_eq!(stats["cached_tensors"], json!(0));
    }
    
    #[test]
    fn test_load_tensor_from_document() {
        let config = TensorConfig::default();
        let engine = TensorEngine::new(config);
        
        let doc = create_test_tensor_document("test_tensor", vec![2, 2], vec![1.0, 2.0, 3.0, 4.0]);
        let tensor = engine.load_tensor_from_document(&doc).unwrap();
        
        assert_eq!(tensor.id, "test_tensor");
        assert_eq!(tensor.shape, vec![2, 2]);
        assert_eq!(tensor.data.len(), 4);
    }
    
    #[test]
    fn test_store_tensor_as_document() {
        let config = TensorConfig::default();
        let engine = TensorEngine::new(config);
        
        let data = ArrayD::from_shape_vec(IxDyn(&[2, 2]), vec![1.0, 2.0, 3.0, 4.0]).unwrap();
        let tensor = TensorData {
            id: "test".to_string(),
            shape: vec![2, 2],
            data,
            created_at: Utc::now(),
            metadata: HashMap::new(),
        };
        
        let doc = engine.store_tensor_as_document(&tensor, Some("tensor_doc".to_string())).unwrap();
        assert_eq!(doc.id, "tensor_doc");
        assert!(doc.data.get("tensor").is_some());
    }
    
    #[test]
    fn test_vector_operations() {
        let config = TensorConfig::default();
        let engine = TensorEngine::new(config);
        
        // Create test vectors
        let tensor1 = TensorData {
            id: "v1".to_string(),
            shape: vec![3],
            data: ArrayD::from_shape_vec(IxDyn(&[3]), vec![1.0, 2.0, 3.0]).unwrap(),
            created_at: Utc::now(),
            metadata: HashMap::new(),
        };
        
        let tensor2 = TensorData {
            id: "v2".to_string(),
            shape: vec![3],
            data: ArrayD::from_shape_vec(IxDyn(&[3]), vec![4.0, 5.0, 6.0]).unwrap(),
            created_at: Utc::now(),
            metadata: HashMap::new(),
        };
        
        // Test vector addition
        let (result, _) = engine.vector_add(&[tensor1.clone(), tensor2.clone()], &HashMap::new()).unwrap();
        let result_tensor = result.unwrap();
        assert_eq!(result_tensor.shape, vec![3]);
        
        // Test dot product
        let (_, scalar_result) = engine.dot_product(&[tensor1.clone(), tensor2.clone()], &HashMap::new()).unwrap();
        let dot_product = scalar_result.unwrap();
        assert_eq!(dot_product, 32.0); // 1*4 + 2*5 + 3*6 = 32
        
        // Test cross product
        let (cross_result, _) = engine.cross_product(&[tensor1, tensor2], &HashMap::new()).unwrap();
        let cross_tensor = cross_result.unwrap();
        assert_eq!(cross_tensor.shape, vec![3]);
    }
}