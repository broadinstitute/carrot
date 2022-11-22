# Do not update this to a newer version unless you can confirm that https://github.com/jupyter/nbconvert/issues/1863 is
# fixed
FROM jupyter/datascience-notebook:lab-3.3.4

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

RUN curl -sSL https://sdk.cloud.google.com | bash

RUN echo "jovyan:jovyan" | chpasswd

ENV PATH $PATH:/home/jovyan/google-cloud-sdk/bin

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
    && pip install ipycytoscape \
    && pip install jupyter_bokeh \
    && pip install jupyterlab_widgets \
    && pip install ipycytoscape

RUN jupyter serverextension enable --py nbresuse --sys-prefix
RUN jupyter nbextension install --py nbresuse --sys-prefix
RUN jupyter nbextension enable --py nbresuse --sys-prefix
RUN jupyter labextension install @jupyter-widgets/jupyterlab-manager --minimize=False
RUN jupyter labextension install jupyterlab-plotly --minimize=False
RUN jupyter labextension install plotlywidget@4.9.0 --minimize=False

RUN R -e "install.packages('ggplot2',dependencies=TRUE, repos='http://cran.rstudio.com/')"

CMD ["start-notebook.sh"]
