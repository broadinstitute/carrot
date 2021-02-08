FROM jupyter/datascience-notebook:python-3.8.5

USER root
RUN apt-get update \
    && apt-get install -y software-properties-common \
    && add-apt-repository universe \
    && apt-get update \
    && apt-get install -y graphviz \
    libgraphviz-dev \
    graphviz-dev \
    pkg-config \
    libz-dev
RUN echo "jovyan:jovyan" | chpasswd

USER jovyan
RUN pip install --upgrade pip \
    && pip install datapackage \
    && pip install folium \
    && pip install --upgrade seaborn \
    && pip install geopandas \
    && pip install tensorflow \
    && pip install nbresuse \
    && pip install umap-learn \
    && pip install scanpy \
    && pip install leidenalg louvain \
    && pip install anndata \
    && pip install joblib \
    && pip install opencv-python \
    && pip install bokeh \
    && pip install "holoviews[recommended]" \
    && pip install graphviz \
    && pip install pysam \
    && pip install plotly==4.9.0 \
    && pip install pygraphviz \
    && pip install parasail \
    && pip install pyabpoa \
    && pip install ipycytoscape

RUN jupyter serverextension enable --py nbresuse --sys-prefix
RUN jupyter nbextension install --py nbresuse --sys-prefix
RUN jupyter nbextension enable --py nbresuse --sys-prefix
RUN jupyter labextension install @jupyter-widgets/jupyterlab-manager --minimize=False
RUN jupyter labextension install @bokeh/jupyter_bokeh --minimize=False
RUN jupyter labextension install jupyterlab-plotly@4.9.0 --minimize=False
RUN jupyter labextension install @jupyter-widgets/jupyterlab-manager plotlywidget@4.9.0 --minimize=False
RUN jupyter labextension install @jupyterlab/toc --minimize=False
RUN jupyter labextension install @jupyter-widgets/jupyterlab-manager jupyter-cytoscape --minimize=False

CMD ["start-notebook.sh"]
